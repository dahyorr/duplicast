import HlsPlayer from "./HlsPlayer"

interface Props { }
const StreamPreview = (props: Props) => {
  return (
    <div className="w-full max-w-[50%] h-auto bg-yellow-500" > StreamPreview
  <HlsPlayer src="http://localhost:1420/preview/playlist.m3u8" />
    </div >
  )
}
export default StreamPreview