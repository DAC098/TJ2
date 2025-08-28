import * as React from "react";
import { EyeOff, Eye } from "lucide-react";

import { Button } from "@/components/ui/button";

import { cn } from "@/utils";

const Input = React.forwardRef<HTMLInputElement, React.ComponentProps<"input">>(
    ({ className, type, ...props }, ref) => {
        return (
            <input
                type={type}
                className={cn(
                    "flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-base ring-offset-background file:border-0 file:bg-transparent file:text-sm file:font-medium file:text-foreground placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 md:text-sm",
                    className
                )}
                ref={ref}
                {...props}
            />
        )
    }
);

Input.displayName = "Input";

const PasswordInput = React.forwardRef<HTMLInputElement, React.ComponentProps<"input">>(({type, className, ...props}, ref) => {
    const [show_password, set_show_password] = React.useState(false);

    return <div className="w-full relative">
        <Input
            type={show_password ? "text" : "password"}
            className={cn("pr-10", className)}
            {...props}
        />
        <Button
            type="button"
            variant="ghost"
            size="icon"
            className="absolute right-0 top-0"
            onClick={() => {
                set_show_password(v => (!v));
            }}
        >
            {show_password ? <EyeOff/> : <Eye/>}
        </Button>
    </div>;
});

PasswordInput.displayName = "PasswordInput";

export { Input, PasswordInput };
