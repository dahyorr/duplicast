import HlsPlayer from "./HlsPlayer"

interface Props {
  previewPort: number;
}
const StreamPreview = ({ previewPort }: Props) => {
  return (
    <div className="w-full max-w-[50%] h-auto bg-black-500" >
      <HlsPlayer src={`http://localhost:${previewPort}/playlist.m3u8`} />
    </div >
  )
}
export default StreamPreview