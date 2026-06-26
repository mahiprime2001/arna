import { createApp } from "vue";
import ConsentApp from "./ConsentApp.vue";
import ChatApp from "./ChatApp.vue";
import "./style.css";

// One frontend, two windows: the consent popup (default) and the chat window
// (opened by the Rust side with ?view=chat).
const view = new URLSearchParams(location.search).get("view");
createApp(view === "chat" ? ChatApp : ConsentApp).mount("#app");
