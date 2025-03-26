import Navbar from "./Navbar";
import StreamInputDetails from "./StreamInputDetails";
import { Divider } from "@heroui/divider";
import StreamPreview from "./StreamPreview";
import Loading from "./Loading";
import useAppState from "../hooks/useAppState";
import StreamDestinations from "./StreamDestinations";

interface Props {

}
const MainPage = ({ }: Props) => {
  const { serversReady } = useAppState()
  if (!serversReady) {
    return <Loading />
  }
  return (
    <div className="container mx-auto">
      <Navbar />

      <div className="flex flex-col gap-4">
        <div className="mt-16">
          <div className="flex gap-2 min-h-[300px]">
            <StreamInputDetails />
            <Divider orientation="vertical" />
            <StreamPreview />
          </div>
        </div>

        <Divider />

        <div>
          <StreamDestinations />
        </div>
      </div>
    </div>
  )
}
export default MainPage