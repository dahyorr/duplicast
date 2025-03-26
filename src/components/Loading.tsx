import { Spinner } from "@heroui/spinner"

const LoadingFullScreen = () => {
  return (
    <div className="flex items-center justify-center h-screen">
      <Spinner size="lg" />
    </div>
  )
}
export default LoadingFullScreen