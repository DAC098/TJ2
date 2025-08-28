import { useLocation, useSearchParams } from "react-router-dom";
import { LoaderCircle, Pencil, Trash } from "lucide-react";
import { useMutation } from "@tanstack/react-query";
import { toast } from "sonner";

import {
    SidebarContent,
    SidebarFooter,
    SidebarGroup,
    SidebarGroupContent,
    SidebarGroupLabel,
    SidebarHeader,
    SidebarMenu,
    SidebarMenuButton,
    SidebarMenuItem,
    SidebarMenuLink,
} from "@/components/ui/sidebar";
import { ApiError, req_api_json } from "@/net";
import { Button } from "@/components/ui/button";
import { useCurrJournal } from "@/components/hooks/journal";
import { Loading } from "@/components/ui/page";
import { H3 } from "@/components/ui/typeography";

type SyncResult = {
    type: "Noop"
} | {
    type: "Queued",
    successful: string[]
}

function JournalSidebar() {
    const location = useLocation();
    const [search_params, _] = useSearchParams();

    const {id, journal, is_loading, is_fetching, error} = useCurrJournal();

    const {mutate: sync, isPending: sync_pending} = useMutation({
        mutationFn: async () => {
            return await req_api_json<SyncResult>("POST", `/journals/${journal?.id}/sync`, {});
        },
        onSuccess: (data) => {
            if (data.type === "Noop") {
                toast("No clients to sync journal");
            } else if (data.type === "Queued") {
                toast(`Syncing journal for clients: ${data.successful.join(", ")}`);
            }
        },
        onError: (err) => {
            if (err instanceof ApiError) {
                toast(`Failed to sync journal: ${err.kind}`);
            } else {
                console.error("failed to sync journal", err);

                toast(`Failed to sync journal`);
            }
        },
    });

    if ((is_loading || is_fetching) && journal == null) {
        return <Loading title="Loading Journal"/>;
    } else if (error != null || id == null || journal == null) {
        return null;
    }

    let entries_path = `/journals/${journal.id}/entries`;

    return <>
        <SidebarHeader className="border-b flex flex-row items-center">
            <H3 className="flex-1 overflow-hidden text-nowrap whitespace-nowrap text-ellipsis" title={journal.name}>
                {journal.name}
            </H3>
            {is_fetching ? <LoaderCircle className="animate-spin"/> : null}
        </SidebarHeader>
        <SidebarContent>
            <SidebarGroup>
                <SidebarGroupLabel>Entries</SidebarGroupLabel>
                <SidebarGroupContent>
                    <SidebarMenu>
                        <SidebarMenuLink
                            title="Calendar"
                            tooltip={"View entries in the journal as a calendar"}
                            path={`${entries_path}?view=calendar`}
                            active={location.pathname.startsWith(entries_path) && search_params.has("view", "calendar")}
                        />
                        <SidebarMenuLink
                            title="Search"
                            tooltip={"Search all available entries for the journal."}
                            path={entries_path}
                            active={location.pathname.startsWith(entries_path) && !search_params.has("view")}
                        />
                    </SidebarMenu>
                </SidebarGroupContent>
            </SidebarGroup>
            <SidebarGroup>
                <SidebarGroupLabel>Properties</SidebarGroupLabel>
                <SidebarGroupContent>
                    <SidebarMenu>
                        <SidebarMenuLink
                            title="Sharing"
                            tooltip={"Shows the list of shares for the journal."}
                            path={`/journals/${journal.id}/share`}
                            active={location.pathname.startsWith(`/journals/${journal.id}/share`)}
                        />
                    </SidebarMenu>
                </SidebarGroupContent>
            </SidebarGroup>
            <SidebarGroup>
                <SidebarGroupLabel>
                    Actions
                </SidebarGroupLabel>
                <SidebarGroupContent>
                    <SidebarMenu>
                        <SidebarMenuLink
                            title="Edit"
                            tooltip="Edit the current journal"
                            path={`/journals/${journal.id}`}
                            active={location.pathname === `/journals/${journal.id}`}
                            icon={<Pencil/>}
                        />
                        <SidebarMenuItem>
                            <SidebarMenuButton
                                disabled={sync_pending}
                                tooltip={{children: "Synchronize journal with attached peers.", hidden: false}}
                                onClick={() => sync()}
                            >
                                Synchronize
                            </SidebarMenuButton>
                        </SidebarMenuItem>
                    </SidebarMenu>
                </SidebarGroupContent>
            </SidebarGroup>
        </SidebarContent>
        <SidebarFooter>
            <Button type="button" variant="destructive">
                <Trash/>Delete
            </Button>
        </SidebarFooter>
    </>
}

export { JournalSidebar };
