import useAppState from "../hooks/useAppState"

interface Props { }
const ConnectionStats = ({ }: Props) => {
  const {  sourceActive } = useAppState()

  return (
    <div className="flex flex-col mb-2">
      <div className="flex items-center gap-2">
        <p>Stream Status:</p>
        <div className="flex items-center gap-2">
          <div className={`h-4 w-4 rounded-full bg-${sourceActive ? "green" : "red"}-500`}></div>
          <span>{sourceActive ? "Active" : "Inactive"}</span>
        </div>
      </div>
    </div>
  )
}
export default ConnectionStats