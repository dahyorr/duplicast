import { Button } from "@heroui/button"
import { Link } from "@heroui/link"
import { NavbarContent, Navbar as HeroNavbar, NavbarBrand, NavbarItem } from "@heroui/navbar"

const Navbar = () => {
  return (
    <HeroNavbar isBordered className="justify-between mx-auto items-center py-4">
      <NavbarBrand className="mr-4">
        <p className="font-bold text-inherit">Duplicast</p>
      </NavbarBrand>

      <NavbarContent justify="end">
        <NavbarItem>
          <Button as={Link} color="primary" href="#" variant="flat">
            Github
          </Button>
        </NavbarItem>
      </NavbarContent>
    </HeroNavbar>
  )
}
export default Navbar