import { Snippet } from "@heroui/snippet"
import useAppState from "../hooks/useAppState"

interface Props {

}
const StreamInputDetails = ({ }: Props) => {
  const { ports, sourceActive } = useAppState()
  return (
    <div className="flex-1 flex flex-col gap-8">
      <div className="flex flex-col mb-2 gap-2">
        <label className='text-2xl font'>Stream URL</label>
        <Snippet >
          {`rtmp://localhost:${ports.rtmp_port}`}
        </Snippet>
      </div>
      {/* 
      <div className="flex flex-col mb-2">
        <label >Stream Key</label>
        <Snippet>
          rtmp://localhost:{rtmpPort}
        </Snippet>
      </div> */}

      <div className="flex flex-col mb-2">
        <div className="flex items-center gap-2">
          <p>Stream Status:</p>
          <div className="flex items-center gap-2">
            <div className={`h-4 w-4 rounded-full bg-${sourceActive ? "green" : "red"}-500`}></div>
            <span>{sourceActive ? "Active" : "Inactive"}</span>
          </div>
        </div>
      </div>
    </div>
  )
}
export default StreamInputDetails