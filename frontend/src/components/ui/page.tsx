import { ReactNode } from "react";

import { cn } from "@/utils";

interface CenterPageProps {
    className?: string,
    children?: ReactNode
}

export function CenterPage({className, children}: CenterPageProps) {
    return <div className={cn("relative max-w-3xl mx-auto my-auto space-y-4", className)}>
        {children}
    </div>;
}
