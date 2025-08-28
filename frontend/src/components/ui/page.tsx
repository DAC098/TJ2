import { PropsWithChildren, ReactNode } from "react";

import { cn } from "@/utils";
import { LoaderCircle } from "lucide-react";
import { H1, H2, P } from "@/components/ui/typeography";

type CenterPageProps = PropsWithChildren<{
    className?: string
}>;

export function CenterPage({className, children}: CenterPageProps) {
    return <div className={cn("relative max-w-3xl mx-auto my-auto space-y-4", className)}>
        {children}
    </div>;
}

type CenterMessageProps = PropsWithChildren<{
    title?: string | ReactNode | null,
}>;

export function CenterMessage({title, children}: CenterMessageProps) {
    let title_node = typeof title === "string" ? <H2>{title}</H2> : title;
    
    return <CenterPage className="flex items-center justify-center h-full">
        <div className="w-1/2 flex flex-col flex-nowrap items-center">
            {title_node}
            {children}
        </div>
    </CenterPage>;
}

export function NothingToSee() {
    return <CenterMessage title="Nothing to see here">
        <P className="text-center">There might be something here in the future but for now it is only the VOID!</P>
    </CenterMessage>;
}

interface LoadingProps {
    title?: string
}

export function Loading({title = "Loading"}: LoadingProps) {
    return <CenterMessage title={<>
        <LoaderCircle className="animate-spin"/>
        <H1 className="scroll-m-20 text-center text-4xl font-extrabold tracking-tight text-balance">{title}</H1>
    </>}/>;
}