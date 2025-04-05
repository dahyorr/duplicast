import { Button } from "@heroui/button";
import { NumberInput } from "@heroui/number-input";
import { Select, SelectItem } from "@heroui/select";
import { addToast } from "@heroui/toast";
import { useState } from "react";
import { Checkbox } from "@heroui/checkbox";

interface Props {
  onModalClose: () => void;
}
interface EncoderSettingsForm {
  encoder: string;
  resolution: string;
  bitrate: number;
  framerate: number;
  preset: string;
  // profile: string;
  // level: string;
  tune: string;
  // gop: number;
  // bufsize: number;
  usePassthrough: boolean;
  matchSourceBitrate: boolean;
  matchSourceFramerate: boolean;
}

const EncoderSetings = ({ onModalClose }: Props) => {
  const [saving, setSaving] = useState(false);
  const [values, setValues] = useState<EncoderSettingsForm>({
    encoder: "libx264",
    resolution: "",
    bitrate: 2500,
    framerate: 30,
    preset: "veryfast",
    // profile: "main",
    // level: "3.1",
    tune: "",
    usePassthrough: false,
    matchSourceBitrate: false,
    matchSourceFramerate: false,
  });

  const handleSubmit = async (e: any) => {
    e.preventDefault();
    setSaving(true);
    try {
      // await invoke("add_relay_target", {
      //   streamKey: key,
      //   url: url,
      //   tag
      // })
      // await getRelayTargets();
      addToast({
        title: "Settings Saved",
        description: "Encoder settings saved successfully",
        color: "success"
      })
    }
    catch (err) {
      console.error(err)
      addToast({
        title: "Error saving settings",
        description: (err as any)?.message,
        color: "danger"
      })
    }
    setSaving(false);
    onModalClose();
  }

  const onChange = (e: any) => {
    const { name, value } = e.target;
    setValues((prev) => ({
      ...prev,
      [name]: value
    }))
  }

  const onValueChange = (key: string, value: any) => {
    setValues((prev) => ({
      ...prev,
      [key]: value
    }))
  }

  return (
    <>
      <form onSubmit={handleSubmit} className="flex flex-col gap-4">

        <div className="flex flex-col gap-2">
          <Select
            id="encoder"
            name="encoder"
            label="Encoder"
            selectedKeys={[values.encoder]}
            onChange={onChange}
            isDisabled={values.usePassthrough}
          >
            <SelectItem key="libx264">libx264</SelectItem>
            {/* <option value="libx265">libx265</option> */}
            {/* <option value="copy">Copy</option> */}
          </Select>

          <Checkbox
            id="usePassthrough"
            name="usePassthrough"
            checked={values.usePassthrough}
            onValueChange={(value) => onValueChange('usePassthrough', value)}
          >Use Passthrough</Checkbox>

        </div>
        <Select
          id="resolution"
          name="resolution"
          label="Resolution"
          selectedKeys={[values.resolution]}
          onChange={onChange}
        >
          <SelectItem key="">Same as source</SelectItem>
          <SelectItem key="1920x1080">1920x1080</SelectItem>
          <SelectItem key="1280x720">1280x720</SelectItem>
          <SelectItem key="640x360">640x360</SelectItem>
        </Select>

        <div className="flex flex-col gap-2">
          <NumberInput
            id="bitrate"
            name="bitrate"
            type="number"
            label="Bitrate (kbps)"
            placeholder="Bitrate (kbps)"
            value={values.bitrate}
            onValueChange={(value) => onValueChange('bitrate', value)}
            isDisabled={values.matchSourceBitrate}
          />

          <Checkbox
            id="matchSourceBitrate"
            name="matchSourceBitrate"
            checked={values.matchSourceBitrate}
            onValueChange={(value) => onValueChange('matchSourceBitrate', value)}
          >Match Source Bitrate</Checkbox>
          {/* same as source */}
        </div>

        <div className="flex flex-col gap-2">
          <NumberInput
            id="framerate"
            name="framerate"
            type="number"
            label="Framerate (fps)"
            placeholder="Framerate (fps)"
            value={values.framerate}
            onValueChange={(value) => onValueChange('framerate', value)}
            isDisabled={values.matchSourceFramerate}
          />
          <Checkbox
            id="matchSourceFramerate"
            name="matchSourceFramerate"
            checked={values.matchSourceFramerate}
            onValueChange={(value) => onValueChange('matchSourceFramerate', value)}
          >Match Source Framerate</Checkbox>
        </div>

        <Select
          id="preset"
          name="preset"
          label="Preset"
          selectedKeys={[values.preset]}
          onChange={onChange}
        >
          <SelectItem key="">Default</SelectItem>
          <SelectItem key="ultrafast">ultrafast</SelectItem>
          <SelectItem key="superfast">superfast</SelectItem>
          <SelectItem key="veryfast">veryfast</SelectItem>
          <SelectItem key="faster">faster</SelectItem>
          <SelectItem key="fast">fast</SelectItem>
          <SelectItem key="medium">medium</SelectItem>
          <SelectItem key="slow">slow</SelectItem>
          <SelectItem key="veryslow">veryslow</SelectItem>
        </Select>

        <Select
          id="tune"
          name="tune"
          label="Tune"
          selectedKeys={[values.tune]}
          onChange={onChange}
        >
          <SelectItem key="">Default</SelectItem>
          <SelectItem key="zerolatency">zerolatency</SelectItem>
          <SelectItem key="film">film</SelectItem>
          <SelectItem key="animation">animation</SelectItem>
          <SelectItem key="grain">grain</SelectItem>
          <SelectItem key="stillimage">stillimage</SelectItem>
          <SelectItem key="psnr">psnr</SelectItem>
          <SelectItem key="ssim">ssim</SelectItem>
          <SelectItem key="fastdecode">fastdecode</SelectItem>
        </Select>

        <div className="flex gap-2 justify-end">
          <Button variant="flat" onPress={onModalClose}>
            Cancel
          </Button>
          <Button color="primary" type={'submit'} isLoading={saving}>
            Save
          </Button>
        </div>
      </form>

    </>
  )
}
export default EncoderSetings