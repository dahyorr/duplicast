// src/components/HlsPlayer.tsx

import React, { useEffect, useRef } from 'react';
import Hls from 'hls.js';

interface HlsPlayerProps {
  src: string;
  autoPlay?: boolean;
  muted?: boolean;
  controls?: boolean;
}

const HlsPlayer: React.FC<HlsPlayerProps> = ({
  src,
  autoPlay = true,
  muted = true,
  controls = true,
}) => {
  const videoRef = useRef<HTMLVideoElement>(null);

  useEffect(() => {
    const video = videoRef.current;

    if (video) {
      if (video.canPlayType('application/vnd.apple.mpegurl')) {
        video.src = src;
      } else if (Hls.isSupported()) {
        const hls = new Hls();
        hls.loadSource(src);
        hls.attachMedia(video);

        hls.on(Hls.Events.ERROR, function (_, data) {
          console.error('❌ HLS error:', data);
        });

        return () => {
          hls.destroy();
        };
      } else {
        console.error('HLS is not supported in this browser.');
      }
    }
  }, [src]);

  return (
    <video
      ref={videoRef}
      style={{ width: '100%', borderRadius: 8 }}
      autoPlay={autoPlay}
      muted={muted}
      controls={controls}
    />
  );
};

export default HlsPlayer;