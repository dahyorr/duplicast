import HlsPlayer from "./HlsPlayer"

interface Props { }
const StreamPreview = (props: Props) => {
  return (
    <div className="w-full max-w-[50%] h-auto bg-black-500" >
      <HlsPlayer src="http://localhost:8787/playlist.m3u8" />
    </div >
  )
}
export default StreamPreview