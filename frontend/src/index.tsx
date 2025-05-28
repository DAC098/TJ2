import { createRoot } from "react-dom/client";

import { MainRouter } from "@/pages";

import "@/media";

function get_window_theme() {
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

function get_theme_preference() {
    if (typeof localStorage != null) {
        let theme = localStorage.getItem("theme");

        return theme == null ? get_window_theme() : theme;
    } else {
        return get_window_theme();
    }
}

if (get_theme_preference() === "dark") {
    document.documentElement.classList.add("dark");
}

document.addEventListener("DOMContentLoaded", () => {
    const root = document.getElementById("root");
    const renderer = createRoot(root);

    renderer.render(<MainRouter/>);
});
