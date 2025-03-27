import useAppState from "../hooks/useAppState"
import HlsPlayer from "./HlsPlayer"

interface Props {

}
const StreamPreview = ({ }: Props) => {
  const { ports: { file_port }, sourceActive } = useAppState()

  if (!sourceActive && file_port !== 0) return (
    <div className="w-full max-w-[50%] h-auto bg-stone-500" >
      <div className="flex items-center justify-center h-full">
        <p className="text-white">No Stream Source Active</p>
      </div>
    </div>
  )

  return (
    <div className="w-full max-w-[50%] h-auto bg-black-500" >
      <HlsPlayer src={`http://localhost:${file_port}/playlist.m3u8`} />
    </div >
  )
}
export default StreamPreview