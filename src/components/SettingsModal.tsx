import { Modal, ModalBody, ModalContent, ModalHeader, useDisclosure } from "@heroui/modal"
import { Tabs, Tab } from "@heroui/tabs";
import EncoderSettings from "./EncoderSetings";
import { Button } from "@heroui/button";
import { MdSettings } from "react-icons/md";

interface Props { }
const SettingsModal = ({ }: Props) => {
  const { isOpen, onOpen, onOpenChange } = useDisclosure()

  return (
    <>
      <Button
        color="primary"
        variant="flat"
        onPress={onOpen}
        isIconOnly
      >
        <MdSettings />
      </Button>
      <Modal isOpen={isOpen} placement="top-center" onOpenChange={onOpenChange}>
        <ModalContent>
          {(onClose) => (
            <>
              <ModalHeader>Settings</ModalHeader>

              <ModalBody>
                <Tabs>
                  {/* <Tab title="General">

                  </Tab> */}
                  <Tab title="Encoder">
                    <EncoderSettings onModalClose={onClose}/>
                  </Tab>
                </Tabs>
              </ModalBody>
            </>
          )}
        </ModalContent>
      </Modal>
    </>
  )
}
export default SettingsModal