import { Button } from "@heroui/button";
import { Input } from "@heroui/input";
import {
  Modal,
  ModalContent,
  ModalHeader,
  ModalBody,
  ModalFooter,
  useDisclosure
} from "@heroui/modal";
import { Select, SelectItem } from "@heroui/select";
import { addToast } from "@heroui/toast";
import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import useAppState from "../hooks/useAppState";

const targetTags = [
  { key: "youtube", label: "YouTube" },
  { key: "twitch", label: "Twitch" },
  { key: "facebook", label: "Facebook" },
  { key: "kick", label: "Kick" },
  { key: "custom", label: "Custom" },
]

const NewRelayTargetModal = () => {
  const [tag, setTag] = useState("");
  const [url, setUrl] = useState("");
  const [key, setKey] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const { isOpen, onOpen, onOpenChange, onClose } = useDisclosure()
  const {getRelayTargets} = useAppState();

  const handleSubmit = async (e: any) => {
    e.preventDefault();
    setSubmitting(true);
    try {
      await invoke("add_relay_target", {
        streamKey: key,
        url: url,
        tag
      })
      await getRelayTargets();
      addToast({
        title: "Relay Target Created",
        description: "Relay target created successfully",
        color: "success"
      })
    }
    catch (err) {
      console.error(err)
      addToast({
        title: "Error creating relay target",
        description: (err as any)?.message,
        color: "danger"
      })
    }
    setSubmitting(false);
    onClose();
  };


  return (
    <>
      <Button onPress={onOpen} color="primary" variant="flat">
        New Target
      </Button>
      <Modal isOpen={isOpen} placement="top-center" onOpenChange={onOpenChange}>
        <ModalContent>
          {(onClose) => (
            <form onSubmit={handleSubmit}>
              <ModalHeader className="flex flex-col gap-1">New Relay Target</ModalHeader>
              <ModalBody >
                <Select
                  id='tag'
                  name='tag'
                  placeholder="Select a tag"
                  label="Tag"
                  isRequired
                  selectedKeys={[tag]}
                  variant="bordered"
                  onChange={(e) => setTag(e.target.value)}
                >
                  {targetTags.map((tag) => (
                    <SelectItem key={tag.key}>{tag.label}</SelectItem>
                  ))}
                </Select>

                <Input
                  id="url"
                  name="url"
                  type="url"
                  label="Stream URL"
                  placeholder="Stream URL"
                  variant="bordered"
                  isRequired
                  onChange={(e) => setUrl(e.target.value)}
                  value={url}
                />
                <Input
                  id="key"
                  name="key"
                  type="text"
                  label="Stream Key"
                  placeholder="Stream Key"
                  variant="bordered"
                  isRequired
                  onChange={(e) => setKey(e.target.value)}
                  value={key}
                />

              </ModalBody>
              <ModalFooter>
                <Button variant="flat" onPress={onClose}>
                  Cancel
                </Button>
                <Button color="primary" type={'submit'} isLoading={submitting}>
                  Create Target
                </Button>
              </ModalFooter>
            </form>
          )}
        </ModalContent>
      </Modal >
    </>
  )
}
export default NewRelayTargetModal