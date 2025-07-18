import { CSSProperties } from "react";
import { Toaster as Sonner, ToasterProps } from "sonner";

export function Toaster(props: ToasterProps) {
    return <Sonner
        theme={"dark"}
        className="toaster group"
        style={{
            "--normal-bg": "var(--popover)",
            "--normal-text": "var(--popover-foreground)",
            "--normal-border": "var(--border)",
        } as CSSProperties}
        {...props}
    />
}