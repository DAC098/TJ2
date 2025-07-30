import { ApiError } from "@/net";
import { cn } from "@/utils";
import { PropsWithChildren, ReactNode } from "react"

type ErrorMsgProps = PropsWithChildren<{
    title?: string | ReactNode,
    className?: string,
}>;

export function ErrorMsg({
    title = "Error",
    className,
    children,
}: ErrorMsgProps) {
    return <div className={cn("flex flex-col items-center justify-center", className)}>
        {typeof title === "string" ?
            <h3 className="text-xl">{title}</h3>
            :
            title
        }
        {children}
    </div>
}

interface MiniErrorMsgProps {
    title?: string,
    message: string,
    className?: string,
}

export function MiniErrorMsg({
    title = "Error",
    message,
    className,
}: MiniErrorMsgProps) {
    return <div className={cn("flex flex-row gap-2", className)}>
        <span className="text-base">{title}</span>
        <span className="text-sm">{message}</span>
    </div>
}

interface ApiErrorMsgProps {
    err: ApiError
}

export function ApiErrorMsg({err}: ApiErrorMsgProps) {
    switch (err.kind) {
        case "ServerError":
            return <ErrorMsg title="Server Error">
                <p>There was a server error when handling your request.</p>
            </ErrorMsg>;
        case "PermissionDenied":
            return <ErrorMsg title="Permission Denied">
                <p>You don't have the necessary permission to perform this action.</p>
            </ErrorMsg>;
        default:
            return <ErrorMsg title={`Error: ${err.kind}`}/>;
    }
}