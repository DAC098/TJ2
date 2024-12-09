import { ReactNode } from "react";

interface CenterPageProps {
    children?: ReactNode
}

export function CenterPage({children}: CenterPageProps) {
    return <div className="max-w-3xl mx-auto my-auto space-y-4">
        {children}
    </div>
}
