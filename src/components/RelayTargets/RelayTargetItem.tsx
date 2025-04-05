import { Card, CardBody } from "@heroui/card"
import { RelayTarget } from "../../typings"
import { Button } from "@heroui/button"
import { Tooltip } from "@heroui/tooltip"
import { Chip } from "@heroui/chip"
import DeleteConfirmationModal from "../DeleteConfirmationModal"
import { FaRegTrashAlt } from "react-icons/fa"
import { FaPowerOff } from "react-icons/fa6"
import { IoPlayOutline, IoStopSharp } from "react-icons/io5"
import { GoAlertFill } from "react-icons/go"
import { IoMdClose } from "react-icons/io"

interface Props {
  target: RelayTarget
  onDelete: (target: RelayTarget) => Promise<void>
  onToggleEnabled: (target: RelayTarget) => void
  onStartRelay: (target: RelayTarget) => Promise<void>
  onStopRelay: (target: RelayTarget) => Promise<void>
}

const RelayTargetItem = ({
  target,
  onDelete,
  onToggleEnabled,
  onStartRelay,
  onStopRelay,
}: Props) => {

  const isEnabled = target.enabled
  const isRelayRunning = target.active
  const isRelayFailed = target.failed

  return (
    <Card className={`relative ${isRelayFailed ? 'border-danger-400' : ''}`}>
      <CardBody className="flex flex-col gap-2">
        <div className="flex items-center justify-between">
          <Chip className="text-muted-foreground font-medium" color='primary' >{target.tag}</Chip>
          <div className="flex gap-1">
            {isRelayFailed && (
              <Chip
                variant="faded"
                color="danger"
                startContent={<GoAlertFill className="h-3 w-3" />}
              >
                Failed
              </Chip>
            )}
            {isRelayRunning && !isRelayFailed && (
              <Chip
                variant="faded"
                color="success"
                startContent={<IoPlayOutline className="h-3 w-3" />}
              >
                Streaming
              </Chip>
            )}
            {isEnabled && !isRelayRunning && !isRelayFailed && (<Chip
              variant="faded"
              color={"secondary"}
              startContent={<IoMdClose />}
            >
              {"Inactive"}
            </Chip>)}
          </div>
        </div>

        {isRelayFailed && (
          <p className="text-sm text-danger-500 mt-1 mb-0">
            {target.errorMessage || "Connection failed. Check your stream settings."}
          </p>
        )}

        <Tooltip content={target.url}>
          <p className="text-sm whitespace-nowrap overflow-hidden text-ellipsis">{target.url}</p>
        </Tooltip>
        <Tooltip content={target.stream_key}>
          <p className="text-sm whitespace-nowrap overflow-hidden text-ellipsis font-mono bg-muted p-1 rounded">{target.stream_key}</p>
        </Tooltip>

        <div className="flex justify-end gap-2 mt-2">
          {!isRelayRunning && isEnabled && !isRelayFailed && (
            <Tooltip content="Start Relay">
              <Button
                variant="flat"
                size="sm"
                color="success"
                onPress={() => onStartRelay(target)}
                aria-label="Start Relay"
                isIconOnly
              >
                <IoPlayOutline className="h-4 w-4" />
              </Button>
            </Tooltip>
          )}

          {
            (isRelayRunning || isRelayFailed) &&
            (
              <Tooltip content="Stop Relay">
                <Button
                  variant="flat"
                  size="sm"
                  color="danger"
                  onPress={() => onStopRelay(target)}
                  aria-label="Stop Relay"
                  isIconOnly
                >
                  <IoStopSharp className="h-4 w-4" />
                </Button>
              </Tooltip>
            )}

          <Tooltip content={isEnabled ? "Disable Target" : "Enable Target"}>
            <Button
              variant={isEnabled ? "flat" : "solid"}
              size="sm"
              color={isEnabled ? "success" : "default"}
              onPress={() => onToggleEnabled?.(target)}
              aria-label={isEnabled ? "Disable Target" : "Enable Target"}
              isIconOnly
            >
              <FaPowerOff className={`h-4 w-4`} />
            </Button>
          </Tooltip>

          <Tooltip content={"Delete"}>
            <DeleteConfirmationModal onConfirm={() => onDelete(target)}>
              {(onOpen) => <Button
                variant="flat"
                size="sm"
                className="text-destructive hover:bg-destructive/10"
                title="Delete"
                isIconOnly
                onPress={onOpen}
              >
                <FaRegTrashAlt className="h-4 w-4" />
              </Button>}
            </DeleteConfirmationModal>
          </Tooltip>
        </div>
      </CardBody>
    </Card>
  )
}

export default RelayTargetItem