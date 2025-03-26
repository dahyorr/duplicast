import { useContext } from "react";
import { AppContext } from "../components/AppState";

const useAppState = () => useContext(AppContext);

export default useAppState;