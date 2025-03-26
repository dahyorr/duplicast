import Navbar from "./Navbar";
import StreamInputDetails from "./StreamInputDetails";
import { Divider } from "@heroui/divider";
import StreamPreview from "./StreamPreview";
import Loading from "./Loading";

interface Props {
  seversReady: boolean;
  ports: { rtmp_port: number, file_port: number }
}
const MainPage = ({ seversReady, ports }: Props) => {
  if (!seversReady) {
    return <Loading />
  }
  return (
    <div className="container mx-auto">
      <Navbar />

      <div className="mt-16">
        <p>Stream Input</p>
        <div className="flex gap-2">
          <StreamInputDetails rtmpPort={ports.rtmp_port}/>
          <Divider orientation="vertical" />
          <StreamPreview previewPort={ports.file_port}/>
        </div>
      </div>
      <p>Stream Destination</p>
      <div>

      </div>
    </div>
  )
}
export default MainPage