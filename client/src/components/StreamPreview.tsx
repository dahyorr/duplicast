import { useEffect, useRef, useState } from "react"
import { Card, CardContent } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Play, Pause, Maximize2, Volume2, VolumeX, Loader2 } from "lucide-react"
import { cn } from "@/lib/utils"
import { sendWebRTCOffer } from "@/api"

interface StreamPreviewProps {
  streamUrl?: string
  streamId?: string
  className?: string
  autoPlay?: boolean
}

export function StreamPreview({ streamUrl, streamId, className, autoPlay = false }: StreamPreviewProps) {
  const videoRef = useRef<HTMLVideoElement>(null)
  const [isPlaying, setIsPlaying] = useState(autoPlay)
  const [isMuted, setIsMuted] = useState(true)
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [peerConnection, setPeerConnection] = useState<RTCPeerConnection | null>(null)

  useEffect(() => {
    if (!streamUrl || !streamId || !videoRef.current) return

    const initWebRTC = async () => {
      setIsLoading(true)
      setError(null)

      try {
        // Create RTCPeerConnection
        const pc = new RTCPeerConnection({
          iceServers: [{ urls: "stun:stun.l.google.com:19302" }],
        })

        // Handle incoming track
        pc.ontrack = (event) => {
          if (videoRef.current && event.streams[0]) {
            videoRef.current.srcObject = event.streams[0]
            setIsLoading(false)
            // Always play: we're here because the user initiated playback (isPlaying=true)
            // or autoPlay is true. Either way, start the video.
            videoRef.current.play().catch((err) => {
              console.error("Play failed:", err)
              setIsPlaying(false)
            })
          }
        }

        // Handle connection state changes
        pc.onconnectionstatechange = () => {
          console.log("Connection state:", pc.connectionState)
          if (pc.connectionState === "failed") {
            setError("Connection failed")
            setIsLoading(false)
          } else if (pc.connectionState === "connected") {
            setIsLoading(false)
          }
        }

        // Create offer and set as local description.
        const offer = await pc.createOffer({
          offerToReceiveVideo: true,
          offerToReceiveAudio: true,
        })
        await pc.setLocalDescription(offer)

        // Wait for all ICE candidates to be gathered before sending the offer.
        // This way the SDP contains the full candidate list and no trickle exchange
        // is needed.
        await new Promise<void>((resolve) => {
          if (pc.iceGatheringState === "complete") { resolve(); return }
          const done = () => { if (pc.iceGatheringState === "complete") resolve() }
          pc.addEventListener("icegatheringstatechange", done)
          // Safety valve: send after 10 s even if gathering isn't marked complete.
          setTimeout(resolve, 10_000)
        })

        // pc.localDescription now includes all gathered candidates.
        const fullOffer = pc.localDescription!

        // Send offer to signaling server and get answer
        const response = await sendWebRTCOffer(streamId, {
          sdp: fullOffer.sdp || "",
          type: fullOffer.type,
        })

        // Set remote description with the answer
        const answer = new RTCSessionDescription({
          sdp: response.data.sdp,
          type: "answer",
        })
        await pc.setRemoteDescription(answer)

        console.log("WebRTC connection established")
        setPeerConnection(pc)
      } catch (err) {
        console.error("WebRTC initialization failed:", err)
        setError(err instanceof Error ? err.message : "Failed to initialize stream")
        setIsLoading(false)
      }
    }

    if (isPlaying) {
      initWebRTC()
    }

    return () => {
      if (peerConnection) {
        peerConnection.close()
        setPeerConnection(null)
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [streamUrl, streamId, isPlaying, autoPlay])

  const togglePlay = () => {
    if (!videoRef.current) return

    if (isPlaying) {
      videoRef.current.pause()
      setIsPlaying(false)
      if (peerConnection) {
        peerConnection.close()
        setPeerConnection(null)
      }
    } else {
      setIsPlaying(true)
    }
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
      </CardContent>
    </Card>
  )
}
