import { Button } from "@heroui/button"
import { NavbarContent, Navbar as HeroNavbar, NavbarBrand, NavbarItem } from "@heroui/navbar"
import { open } from "@tauri-apps/plugin-shell"

import { FaGithub } from "react-icons/fa6"

const Navbar = () => {
  return (
    <HeroNavbar isBordered>
      <NavbarBrand>
        <p className="font-bold text-inherit">Duplicast</p>
      </NavbarBrand>

      <NavbarContent justify="end">
        <NavbarItem>
          <Button
            color="primary"
            variant="flat"
            onPress={() => open("https://github.com/dahyorr/duplicast")}
            isIconOnly
          >
            <FaGithub />
          </Button>
        </NavbarItem>
      </NavbarContent>
    </HeroNavbar>
  )
}
export default Navbar