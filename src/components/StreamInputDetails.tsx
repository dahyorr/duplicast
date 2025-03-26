import { Snippet } from "@heroui/snippet"

interface Props { rtmpPort: number }
const StreamInputDetails = ({ rtmpPort }: Props) => {
  return (
    <div className="flex-1">
      <div className="flex flex-col mb-2">
        <label>Stream URL</label>
        <Snippet >
          rtmp://localhost:{rtmpPort}
        </Snippet>
      </div>

      <div className="flex flex-col mb-2">
        <label >Stream Key</label>
        <Snippet>
          rtmp://localhost:{rtmpPort}
        </Snippet>
      </div>
    </div>
  )
}
export default StreamInputDetails