import { cn } from "@/utils";
import { ComponentProps, forwardRef } from "react"

export const H1 = forwardRef<HTMLHeadingElement, ComponentProps<"h1">>(({className, ...props}, ref) => {
    return <h1
        ref={ref}
        className={cn("scroll-m-20 text-center text-4xl font-extrabold tracking-tight text-balance", className)}
        {...props}
    />
});
H1.displayName = "H1";

export const H2 = forwardRef<HTMLHeadingElement, ComponentProps<"h2">>(({className, ...props}, ref) => {
    return <h2
        ref={ref}
        className={cn("scroll-m-20 border-b pb-2 text-3xl font-semibold tracking-tight first:mt-0", className)}
        {...props}
    />;
});
H2.displayName = "H2";

export const H3 = forwardRef<HTMLHeadingElement, ComponentProps<"h3">>(({className, ...props}, ref) => {
    return <h3
        ref={ref}
        className={cn("scroll-m-20 text-2xl font-semibold tracking-tight", className)}
        {...props}
    />;
});
H3.displayName = "H3";

export const H4 = forwardRef<HTMLHeadingElement, ComponentProps<"h4">>(({className, ...props}, ref) => {
    return <h4
        ref={ref}
        className={cn("scroll-m-20 text-xl font-semibold tracking-tight", className)}
        {...props}
    />;
});
H4.displayName = "H4";

export const P = forwardRef<HTMLParagraphElement, ComponentProps<"p">>(({className, ...props}, ref) => {
    return <p ref={ref} className={cn("leading-7 [&:not(:first-child)]:mt-6", className)} {...props}/>;
});
P.displayName = "P";