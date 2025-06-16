export function get_window_theme() {
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export function get_theme_preference() {
    if (typeof localStorage != null) {
        let theme = localStorage.getItem("theme");

        return theme == null ? get_window_theme() : theme;
    } else {
        return get_window_theme();
    }
}

export function switch_theme(theme: "light" | "dark") {
    let current = get_theme_preference();

    if (get_theme_preference() === theme) {
        return;
    }

    document.documentElement.classList.remove(current);
    document.documentElement.classList.add(theme);

    if (typeof localStorage != null) {
        localStorage.setItem("theme", theme);
    }
}

export function toggle_theme() {
    let current = get_theme_preference();

    if (current === "dark") {
        switch_theme("light");
    } else {
        switch_theme("dark");
    }
}

export function init_theme() {
    if (get_theme_preference() === "dark") {
        document.documentElement.classList.add("dark");
    }
}
