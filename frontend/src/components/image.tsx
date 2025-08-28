import { SquareArrowOutUpRight } from "lucide-react";
import { Root as VisuallyHidden } from "@radix-ui/react-visually-hidden";

import { Button } from "@/components/ui/button";
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogTitle,
    DialogTrigger,
} from "@/components/ui/dialog";
import { useObjectUrl } from "@/hooks";

export interface ViewImageProps {
    src: string | File | Blob
}

export function ViewImage({src}: ViewImageProps) {
    let url = useObjectUrl(src);

    return <Dialog>
        <DialogTrigger asChild>
            <Button type="button" variant="secondary" size="icon">
                <SquareArrowOutUpRight/>
            </Button>
        </DialogTrigger>
        <VisuallyHidden>
            <DialogTitle>View Image</DialogTitle>
            <DialogDescription>
                Displays images attached to a journal entry
            </DialogDescription>
        </VisuallyHidden>
        <DialogContent>
            {url != null ? <img src={url} onLoad={() => {}}/> : null}
        </DialogContent>
    </Dialog>;
}
