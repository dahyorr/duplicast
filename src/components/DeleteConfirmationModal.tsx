import React, { useState } from 'react';
import {
  Modal,
  ModalContent,
  ModalHeader,
  ModalBody,
  ModalFooter,
  useDisclosure
} from "@heroui/modal";
import { Button } from '@heroui/button';

interface DeleteConfirmationModalProps {
  onConfirm: () => Promise<void>;
  title?: string;
  message?: string;
  itemName?: string;
  children: ((onOpen: () => void) => React.ReactNode);
}

const DeleteConfirmationModal: React.FC<DeleteConfirmationModalProps> = ({
  onConfirm,
  title = "Confirm Deletion",
  message = "Are you sure you want to delete this item?",
  itemName,
  children
}) => {
  const { isOpen, onOpen, onOpenChange } = useDisclosure();
  const [deleting, setDeleting] = useState(false);

  const handleConfirm = async (onClose: () => void) => {
    setDeleting(true);
    try {
      await onConfirm();
    } catch (error) {
      console.error("Error during deletion:", error);
    }
    setDeleting(false);
    onClose();
  };


  return (
    <>
      <span onClick={onOpen} className="cursor-pointer inline-block">
        {children(onOpen)}
      </span>

      <Modal isOpen={isOpen} onOpenChange={onOpenChange}>
        <ModalContent>
          {(onClose) => (
            <>
              <ModalHeader className="flex flex-col gap-1">{title}</ModalHeader>
              <ModalBody>
                <p>
                  {itemName ? `${message.replace('this item', `"${itemName}"`)}` : message}
                </p>
                <p>
                  This action cannot be undone.
                </p>
              </ModalBody>
              <ModalFooter>
                <Button color="danger" variant="light" onPress={onClose}>
                  Cancel
                </Button>
                <Button color="primary" onPress={() => handleConfirm(onClose)} isLoading={deleting}>
                  Delete
                </Button>
              </ModalFooter>
            </>
          )}
        </ModalContent>
      </Modal>
    </>
  );
};

export default DeleteConfirmationModal;
