import { useEffect, useRef, useState } from "react"
import { Card, CardContent } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Play, Pause, Maximize2, Volume2, VolumeX, Loader2 } from "lucide-react"
import { cn } from "@/lib/utils"
import { sendWebRTCOffer, sendWebRTCHangup, getFlvUrl } from "@/api"
import { useConfig } from "@/hooks"
import type { FlvPlayer } from "flv.js"

interface StreamPreviewProps {
  streamUrl?: string
  streamId?: string
  className?: string
  autoPlay?: boolean
}

type PreviewMode = "flv" | "webrtc"

export function StreamPreview({ streamUrl, streamId, className, autoPlay = false }: StreamPreviewProps) {
  const videoRef = useRef<HTMLVideoElement>(null)
  const [mode, setMode] = useState<PreviewMode>("flv")
  const [isPlaying, setIsPlaying] = useState(autoPlay)
  const [isMuted, setIsMuted] = useState(true)
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const { data: config } = useConfig()

  // HTTP-FLV playback (default). Far simpler than WebRTC - just an HTTP stream
  // decoded via Media Source Extensions, no ICE/DTLS/codec negotiation.
  useEffect(() => {
    if (mode !== "flv" || !streamUrl || !streamId || !videoRef.current || !isPlaying) return

    let cancelled = false
    let player: FlvPlayer | null = null
    const video = videoRef.current
    const onCanPlay = () => { if (!cancelled) setIsLoading(false) }

    setIsLoading(true)
    setError(null)

    // Loaded on demand - most previews use FLV, but no reason to ship it in the
    // main bundle when the WebRTC-only path doesn't need it at all.
    import("flv.js").then(({ default: flvjs }) => {
      if (cancelled) return
      if (!flvjs.isSupported()) {
        setError("HTTP-FLV playback isn't supported in this browser")
        return
      }

      player = flvjs.createPlayer({ type: "flv", url: getFlvUrl(streamId), isLive: true })
      player.attachMediaElement(video)

      player.on(flvjs.Events.ERROR, (...args: unknown[]) => {
        if (cancelled) return
        console.error("flv.js error:", ...args)
        setError("Playback error")
        setIsLoading(false)
      })

      video.addEventListener("canplay", onCanPlay)

      player.load()
      player.play().catch((err) => {
        if (cancelled) return
        console.error("Play failed:", err)
        setIsPlaying(false)
      })
    })

    return () => {
      cancelled = true
      video.removeEventListener("canplay", onCanPlay)
      player?.pause()
      player?.unload()
      player?.detachMediaElement()
      player?.destroy()
    }
  }, [mode, streamUrl, streamId, isPlaying])

  // WebRTC playback (lower latency, more moving parts - offered as an alternative).
  useEffect(() => {
    if (mode !== "webrtc" || !streamUrl || !streamId || !videoRef.current || !isPlaying) return

    // Effect-scoped (not React state) so this run's cleanup always sees exactly
    // what this run created - React StrictMode double-invokes effects in dev
    // mode (mount -> cleanup -> mount again), and a stale/shared reference here
    // would either miss cleanup or tear down the wrong connection.
    let cancelled = false
    let pc: RTCPeerConnection | null = null
    let sessionId: string | null = null

    const initWebRTC = async () => {
      setIsLoading(true)
      setError(null)

      try {
        const stunServer = config?.stun_server || "stun.l.google.com:19302"
        const localPc = new RTCPeerConnection({
          iceServers: [{ urls: `stun:${stunServer}` }],
        })
        pc = localPc

        localPc.ontrack = (event) => {
          if (cancelled) return
          if (videoRef.current && event.streams[0]) {
            videoRef.current.srcObject = event.streams[0]
            setIsLoading(false)
            videoRef.current.play().catch((err) => {
              console.error("Play failed:", err)
              setIsPlaying(false)
            })
          }
        }

        localPc.onconnectionstatechange = () => {
          if (cancelled) return
          if (localPc.connectionState === "failed") {
            setError("Connection failed")
            setIsLoading(false)
          } else if (localPc.connectionState === "connected") {
            setIsLoading(false)
          }
        }

        const offer = await localPc.createOffer({
          offerToReceiveVideo: true,
          offerToReceiveAudio: true,
        })
        if (cancelled) { localPc.close(); return }
        await localPc.setLocalDescription(offer)

        // Wait for all ICE candidates to be gathered before sending the offer.
        // This way the SDP contains the full candidate list and no trickle exchange
        // is needed.
        await new Promise<void>((resolve) => {
          if (localPc.iceGatheringState === "complete") { resolve(); return }
          const done = () => { if (localPc.iceGatheringState === "complete") resolve() }
          localPc.addEventListener("icegatheringstatechange", done)
          // Safety valve: send after 10 s even if gathering isn't marked complete.
          setTimeout(resolve, 10_000)
        })
        if (cancelled) { localPc.close(); return }

        // pc.localDescription now includes all gathered candidates.
        const fullOffer = localPc.localDescription!

        // Send offer to signaling server and get answer
        const response = await sendWebRTCOffer(streamId, {
          sdp: fullOffer.sdp || "",
          type: fullOffer.type,
        })

        if (cancelled) {
          // A server-side session now exists for an effect run we no longer want -
          // hang it up immediately instead of leaking it until the stream ends.
          sendWebRTCHangup(streamId, response.data.session_id).catch(() => {})
          localPc.close()
          return
        }
        sessionId = response.data.session_id

        // Set remote description with the answer
        const answer = new RTCSessionDescription({
          sdp: response.data.sdp,
          type: "answer",
        })
        await localPc.setRemoteDescription(answer)
      } catch (err) {
        if (!cancelled) {
          console.error("WebRTC initialization failed:", err)
          setError(err instanceof Error ? err.message : "Failed to initialize stream")
          setIsLoading(false)
        }
      }
    }

    initWebRTC()

    return () => {
      cancelled = true
      if (sessionId) {
        sendWebRTCHangup(streamId, sessionId).catch(() => {
          // Best effort - the session will still be cleaned up server-side
          // when the underlying stream ends.
        })
      }
      pc?.close()
    }
    // config is intentionally not a dependency: if it resolves/changes after this
    // effect already started negotiating, we don't want to tear down and restart
    // an in-progress or already-connected preview over a STUN server change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mode, streamUrl, streamId, isPlaying])

  const togglePlay = () => {
    if (!videoRef.current) return

    if (isPlaying) {
      videoRef.current.pause()
      setIsPlaying(false)
    } else {
      setIsPlaying(true)
    }
  }

  const switchMode = (next: PreviewMode) => {
    if (next === mode) return
    setError(null)
    setMode(next)
  }

  const toggleMute = () => {
    if (videoRef.current) {
      videoRef.current.muted = !isMuted
      setIsMuted(!isMuted)
    }
  }

  const toggleFullscreen = () => {
    if (videoRef.current) {
      if (document.fullscreenElement) {
        document.exitFullscreen()
      } else {
        videoRef.current.requestFullscreen()
      }
    }
  }

  if (!streamUrl || !streamId) {
    return (
      <Card className={cn("overflow-hidden", className)}>
        <CardContent className="flex items-center justify-center bg-muted p-0 aspect-video">
          <div className="text-center text-muted-foreground">
            <p className="text-sm">No active stream</p>
          </div>
        </CardContent>
      </Card>
    )
  }

  return (
    <Card className={cn("overflow-hidden", className)}>
      <CardContent className="relative p-0 group">
        {/* Video Element */}
        <video
          ref={videoRef}
          className="w-full aspect-video bg-black"
          muted={isMuted}
          playsInline
        />

        {/* Loading Overlay */}
        {isLoading && (
          <div className="absolute inset-0 flex items-center justify-center bg-black/50">
            <div className="flex flex-col items-center gap-2 text-white">
              <Loader2 className="h-8 w-8 animate-spin" />
              <p className="text-sm">Connecting to stream...</p>
            </div>
          </div>
        )}

        {/* Error Overlay */}
        {error && (
          <div className="absolute inset-0 flex items-center justify-center bg-black/80">
            <div className="text-center text-white px-4">
              <p className="text-sm mb-2">{error}</p>
              <Button size="sm" variant="secondary" onClick={() => setIsPlaying(true)}>
                Retry
              </Button>
            </div>
          </div>
        )}

        {/* Controls Overlay */}
        <div className="absolute inset-0 bg-linear-to-t from-black/60 via-transparent to-transparent opacity-0 group-hover:opacity-100 transition-opacity">
          <div className="absolute bottom-0 left-0 right-0 p-4 flex items-center gap-2">
            {/* Play/Pause */}
            <Button size="icon" variant="ghost" className="h-8 w-8 text-white" onClick={togglePlay}>
              {isPlaying ? <Pause className="h-4 w-4" /> : <Play className="h-4 w-4" />}
            </Button>

            {/* Volume */}
            <Button size="icon" variant="ghost" className="h-8 w-8 text-white" onClick={toggleMute}>
              {isMuted ? <VolumeX className="h-4 w-4" /> : <Volume2 className="h-4 w-4" />}
            </Button>

            {/* Status Badge */}
            <Badge variant="outline" className="ml-auto border-white/20 bg-black/40 text-white text-xs">
              {isLoading ? "Connecting..." : isPlaying ? "LIVE" : "Paused"}
            </Badge>

            {/* Fullscreen */}
            <Button size="icon" variant="ghost" className="h-8 w-8 text-white" onClick={toggleFullscreen}>
              <Maximize2 className="h-4 w-4" />
            </Button>
          </div>
        </div>

        {/* Mode toggle - always visible, not just on hover. Rendered last so it
            stacks above the Controls Overlay, which covers the whole card and
            would otherwise swallow clicks on this corner even at opacity 0. */}
        <div className="absolute top-2 right-2 flex overflow-hidden rounded-md border border-white/20 bg-black/50 text-xs">
          <button
            type="button"
            onClick={() => switchMode("flv")}
            className={cn(
              "px-2 py-1 transition-colors",
              mode === "flv" ? "bg-primary text-primary-foreground" : "text-white/70 hover:text-white"
            )}
          >
            FLV
          </button>
          <button
            type="button"
            onClick={() => switchMode("webrtc")}
            className={cn(
              "px-2 py-1 transition-colors",
              mode === "webrtc" ? "bg-primary text-primary-foreground" : "text-white/70 hover:text-white"
            )}
          >
            WebRTC
          </button>
        </div>
      </CardContent>
    </Card>
  )
}
