import { Toaster as Sonner, ToasterProps } from "sonner";

export function Toaster(props: ToasterProps) {
    return <Sonner
        theme={"dark"}
        className="toaster group"
        toastOptions={{
            style: {
                backgroundColor: "hsl(var(--popover))",
                color: "hsl(var(--popover-foreground))",
                borderColor: "hsl(var(--border))",
            }
        }}
        {...props}
    />
}