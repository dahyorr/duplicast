import NewRelayTargetModal from "../NewRelayTargetModal"
import useAppState from "../../hooks/useAppState"
import RelayTargetItem from "./RelayTargetItem"
import { RelayTarget } from "../../typings"
import { addToast } from "@heroui/toast"
import { invoke } from "@tauri-apps/api/core"
import { Button } from "@heroui/button"

interface Props { }
const RelayTargets = ({ }: Props) => {
  const { relayTargets, getRelayTargets } = useAppState()

  const onToggleEnabled = async (target: RelayTarget) => {
    const isEnabled = target.enabled
    try {
      await invoke("toggle_relay_target", { id: target.id, active: !isEnabled })
      addToast({
        title: `Relay Target ${!isEnabled ? "Activated" : "Deactivated"}`,
        description: `Relay target ${target.tag} has been ${!isEnabled ? "activated" : "deactivated"}`,
        color: "success"
      })
    }
    catch (err) {
      console.error(err)
      addToast({
        title: "Error toggling relay target",
        description: (err as any)?.message,
        color: "danger"
      })
    }
    await getRelayTargets()
  }

  const onDelete = async (target: RelayTarget) => {
    try {
      await invoke("remove_relay_target", { id: target.id })
      addToast({
        title: "Relay Target Deleted",
        description: "Relay target deleted successfully",
        color: "success"
      })
    }
    catch (err) {
      console.error(err)
      addToast({
        title: "Error deleting relay target",
        description: (err as any)?.message,
        color: "danger"
      })
    }
    await getRelayTargets()
  }

  const startAllRelayTargets = async () => {
    try {
      await invoke("start_all_relays")
      addToast({
        title: "Relay Targets Started",
        description: "All relay targets started successfully",
        color: "success"
      })
    }
    catch (err) {
      console.error(err)
      addToast({
        title: "Error starting relay targets",
        description: (err as any)?.message,
        color: "danger"
      })
    }
  }

  const stopAllRelayTargets = async () => {
    try {
      await invoke("stop_all_relays")
      addToast({
        title: "Relay Targets Stopped",
        description: "All relay targets stopped successfully",
        color: "success"
      })
    }
    catch (err) {
      console.error(err)
      addToast({
        title: "Error stopping relay targets",
        description: (err as any)?.message,
        color: "danger"
      })
    }
  }

  const onStartRelay = async (target: RelayTarget) => {
    try {
      await invoke("start_relay", { id: target.id })
      addToast({
        title: "Relay Target Started",
        description: "Relay target started successfully",
        color: "success"
      })
    }
    catch (err) {
      console.error(err)
      addToast({
        title: "Error starting relay target",
        description: (err as any)?.message,
        color: "danger"
      })
    }
  }

  const onStopRelay = async (target: RelayTarget) => {
    try {
      await invoke("stop_relay", { id: target.id })
      addToast({
        title: "Relay Target Stopped",
        description: "Relay target stopped successfully",
        color: "success"
      })
    }
    catch (err) {
      console.error(err)
      addToast({
        title: "Error stopping relay target",
        description: (err as any)?.message,
        color: "danger"
      })
    }
  }


  const relays = Object.values(relayTargets)

  return (
    <div className=" flex flex-col gap-4">
      <div className="flex justify-between items-center">
        <p>Relay Targets</p>
        <div className="flex gap-2">
          <NewRelayTargetModal />
          <Button
            variant="flat"
            color="primary"
            onPress={startAllRelayTargets}
            className="flex items-center gap-2"
          >
            Start All
          </Button>
          <Button
            variant="flat"
            color="danger"
            onPress={stopAllRelayTargets}
            className="flex items-center gap-2"
          >
            Stop All
          </Button>
        </div>
      </div>

      {relays.length < 1 && (<div className="flex flex-col gap-2">
        <p className="text-muted-foreground text-center">No relay targets yet</p>
        <p className="text-muted-foreground text-center">Click the button above to create one</p>
      </div>)}

      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        {relays.map((target) => (
          <RelayTargetItem
            key={target.id}
            target={target}
            onToggleEnabled={onToggleEnabled}
            onDelete={onDelete}
            onStartRelay={onStartRelay}
            onStopRelay={onStopRelay}
          />
        ))}
      </div>


    </div>
  )
}
export default RelayTargets