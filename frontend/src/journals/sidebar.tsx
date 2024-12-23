import { useState, useEffect, Fragment } from "react";
import { Link, useLocation } from "react-router-dom";
import { Plus, Save, Trash, RefreshCcw, Search, Pencil, EllipsisVertical } from "lucide-react";
import { format } from "date-fns";

import { Button } from "@/components/ui/button";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import {
    Sidebar,
    SidebarContent,
    SidebarFooter,
    SidebarGroup,
    SidebarGroupLabel,
    SidebarGroupContent,
    SidebarHeader,
    SidebarMenu,
    SidebarMenuItem,
    SidebarMenuButton,
    useSidebar,
} from "@/components/ui/sidebar";
import {
    JournalPartial,
    JournalFull,
    get_journals,
    get_journal,
} from "@/journals/api";
import { cn } from "@/utils";

interface JournalOptionsProps {
    journals_id: number
}

function JournalOptions({journals_id}: JournalOptionsProps) {
    return <div className="absolute top-2 right-2">
        <Link to={`/journals/${journals_id}/entries/new`}>
            <Button type="button" variant="ghost" size="icon" title="New Entry">
                <Plus />
            </Button>
        </Link>
        <DropdownMenu>
            <DropdownMenuTrigger asChild>
                <Button type="button" variant="ghost" size="icon">
                    <EllipsisVertical />
                </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent>
                <Link to={`/journals/${journals_id}`}>
                    <DropdownMenuItem>
                        <Pencil />Edit
                    </DropdownMenuItem>
                </Link>
                <DropdownMenuSeparator />
                <DropdownMenuItem>
                    <Trash />Delete
                </DropdownMenuItem>
            </DropdownMenuContent>
        </DropdownMenu>
    </div>;
}

function JournalSidebar() {
    const location = useLocation();

    let [loading, set_loading] = useState(false);
    let [data, set_data] = useState<JournalPartial[]>([]);

    useEffect(() => {
        set_loading(true);

        get_journals().then(list=> {
            if (list == null) {
                return;
            }

            set_data(list);
        }).catch(err => {
            console.error("failed to load journal list");
        }).finally(() => {
            set_loading(false);
        });
    }, []);

    let data_elements = data.map((journal, index) => {
        let created_ts = new Date(journal.created);
        let updated_ts = journal.updated != null ? new Date(journal.updated) : null;
        let path_prefix = `/journals/${journal.id}`;

        return <div
            key={journal.id}
            className={cn(
                "hover:bg-sidebar-accent hover:text-sidebar-accent-foreground border-b relative",
                {"bg-sidebar-accent text-sidebar-accent-forground": location.pathname.startsWith(path_prefix)},
            )}
        >
            <Link to={`${path_prefix}/entries`}>
                <div className="p-4 space-y-2">
                    <h2 className="text-xl w-1/2 truncate font-semibold">{journal.name}</h2>
                    <p>{journal.description}</p>
                    <div className="flex flex-row flex-nowrap">
                        <span title={format(created_ts, "Pp")} className="pr-2">
                            C: {format(created_ts, "yyyy/MM/dd")}
                        </span>
                        {updated_ts != null ?
                            <span title={format(updated_ts, "Pp")} className="pl-2 border-l">
                                U: {format(updated_ts, "yyyy/MM/dd")}
                            </span>
                            :
                            null
                        }
                    </div>
                </div>
            </Link>
            <JournalOptions journals_id={journal.id}/>
        </div>;
    });

    return <>
        <SidebarHeader className="border-b">
            <div className="w-full relative">
                <Input type="text" className="pr-10"/>
                <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    className="absolute right-0 top-0"
                    onClick={() => {}}
                >
                    <Search />
                </Button>
            </div>
            <SidebarMenu>
                <SidebarMenuItem>
                    <Link to="/journals/new">
                        <SidebarMenuButton>
                            <Plus />
                            <span>New Journal</span>
                        </SidebarMenuButton>
                    </Link>
                </SidebarMenuItem>
            </SidebarMenu>
        </SidebarHeader>
        <SidebarContent>
            <SidebarGroup className="p-0">
                <SidebarGroupContent className="flex flex-col">
                    {data_elements}
                </SidebarGroupContent>
            </SidebarGroup>
        </SidebarContent>
    </>
}

export { JournalSidebar };
