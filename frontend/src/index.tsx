import { createRoot } from "react-dom/client";

import { MainRouter } from "@/pages";
import { init_theme } from "@/theme";

import "@/media";

init_theme();

document.addEventListener("DOMContentLoaded", () => {
    const root = document.getElementById("root");

    if (root == null) {
        console.error("failed to find root element");
        return;
    }

    const renderer = createRoot(root);

    renderer.render(<MainRouter/>);
});
