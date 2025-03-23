import { Snippet } from "@heroui/snippet"

interface Props { }
const StreamInputDetails = (props: Props) => {
  return (
    <div>
      <div className="flex flex-col mb-2">
        <label>Stream URL</label>
        <Snippet >
          rtmp://localhost:8080
        </Snippet>
      </div>

      <div className="flex flex-col mb-2">
        <label c>Stream Key</label>
        <Snippet>
          rtmp://localhost:8080
        </Snippet>
      </div>
    </div>
  )
}
export default StreamInputDetails