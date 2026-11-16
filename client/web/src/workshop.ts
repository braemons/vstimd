import { installWorkshopRig } from "../maze-workshop/app.mjs";
import { WorkshopSession } from "./workshopApi.js";
const origin = `${location.protocol === "https:" ? "wss:" : "ws:"}//${location.host}`;
const session = new WorkshopSession(origin);
installWorkshopRig(session);
window.addEventListener("pagehide", () => session.close());
