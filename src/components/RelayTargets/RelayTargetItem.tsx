import { Card, CardBody } from "@heroui/card"
import { RelayTarget } from "../../typings"
import { Button } from "@heroui/button"
import { Trash2, Power, Check, X } from "lucide-react"

import { Tooltip } from "@heroui/tooltip"
import { Chip } from "@heroui/chip"
import DeleteConfirmationModal from "../DeleteConfirmationModal"

interface Props {
  target: RelayTarget
  onDelete: (target: RelayTarget) => Promise<void>
  onToggleActive: (target: RelayTarget) => void
}

// const tagColors = {
//   youtube: "bg-red-500",
//   twitch: "bg-purple-500",
//   facebook: "bg-blue-500",
//   kick: "bg-green-500",
//   custom: "bg-gray-500",
// }


const RelayTargetItem = ({ target, onDelete, onToggleActive }: Props) => {
  // Default to active if not specified
  const isActive = target.enabled
  console.log(target)

  return (
    <Card className="relative">
      <CardBody className="flex flex-col gap-2">
        <div className="flex items-center justify-between">
          <Chip className="text-muted-foreground font-medium" color='primary' >{target.tag}</Chip>
          <Chip
            variant="faded"
            color={isActive ? "success" : "secondary"}
            startContent={isActive ? <Check /> : <X />}
          >
            {isActive ? "Active" : "Inactive"}
          </Chip>
        </div>
        <p className="text-sm break-all">{target.url}</p>
        <p className="text-sm break-all font-mono bg-muted p-1 rounded">{target.stream_key}</p>

        <div className="flex justify-end gap-2 mt-2">

          <Tooltip content={isActive ? "Deactivate" : "Activate"}>
            <Button
              variant="flat"
              size="sm"
              onPress={() => onToggleActive?.(target)}
              aria-label={isActive ? "Deactivate" : "Activate"}
            >
              <Power className={`h-4 w-4 ${isActive ? "text-green-500" : "text-gray-500"}`} />
            </Button>
          </Tooltip>
          <Tooltip content={"Delete"}>

            <DeleteConfirmationModal onConfirm={() => onDelete(target)}>
              {(onOpen) => <Button
                variant="flat"
                size="sm"
                className="text-destructive hover:bg-destructive/10"
                title="Delete"
                onPress={onOpen}
              >
                <Trash2 className="h-4 w-4" />
              </Button>}
            </DeleteConfirmationModal>
          </Tooltip>
        </div>
      </CardBody>
    </Card>
  )
}

export default RelayTargetItem