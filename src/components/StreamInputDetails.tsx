import { Snippet } from "@heroui/snippet"
import useAppState from "../hooks/useAppState"
import ConnectionStats from "./ConnectionStats"

interface Props {

}
const StreamInputDetails = ({ }: Props) => {
  const { ports, ips } = useAppState()
  return (
    <div className="flex-1 flex flex-col gap-8">
      <div className="flex flex-col mb-2 gap-2">
        <label className='text-2xl font'>Stream URL</label>
        {ips.map((ip) => (
          <Snippet key={ip}>
            {`rtmp://${ip}:${ports.rtmp_port}`}
          </Snippet>
        ))}

      </div>
      <ConnectionStats />
    </div>
  )
}
export default StreamInputDetails