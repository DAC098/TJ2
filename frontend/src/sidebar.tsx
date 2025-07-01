import { Link, Routes, Route, useNavigate, useLocation } from "react-router-dom";
import {
    ChevronsUpDown,
    LogOut,
    Notebook,
    Settings,
    EarthLock,
    PanelLeft,
    PanelLeftClose,
} from "lucide-react";

import {
    Avatar,
    AvatarFallback,
    AvatarImage,
} from "@/components/ui/avatar";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuLabel,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
    Sidebar,
    SidebarContent,
    SidebarGroup,
    SidebarGroupLabel,
    SidebarGroupContent,
    SidebarHeader,
    SidebarMenu,
    SidebarMenuItem,
    SidebarMenuButton,
    SidebarMenuLink,
    useSidebar,
} from "@/components/ui/sidebar";

import { JournalSidebar } from "@/sidebar/journals";
import { AdminSidebar } from "@/sidebar/admin";
import { SettingsSidebar } from "@/sidebar/settings";
import { toggle_theme } from "@/theme";
import { ReactElement, useEffect, useState } from "react";
import { cn } from "./utils";
import { Button } from "@/components/ui/button";

async function send_logout() {
    let res = await fetch("/logout", {
        method: "POST"
    });

    if (res.status != 200) {
        throw new Error("failed to logout user");
    }
}

interface UserBadgeProps {
    name: string,
    email: string,
    avatar?: string,
    show_details?: boolean
}

function UserBadge({
    name,
    email,
    avatar,
    show_details = false
}: UserBadgeProps) {
    return <>
        <Avatar className="h-8 w-8 rounded-lg">
            {avatar != null ? <AvatarImage src={avatar} alt={name} /> : null}
            <AvatarFallback className="rounded-lg">TJ2</AvatarFallback>
        </Avatar>
        {show_details ?
            <div className="grid flex-1 text-left text-sm leading-tight">
                <span className="truncate font-semibold">{name}</span>
                <span className="truncate text-xs">{email}</span>
            </div>
            :
            null
        }
    </>;
}

interface UserMenuProps {
    collapsed: boolean
}

function UserMenu({collapsed}: UserMenuProps) {
    const { isMobile } = useSidebar();
    const navigate = useNavigate();

    let name = "The Dude";
    let email = "the_dude@laboski.drink";

    return <SidebarMenu>
        <SidebarMenuItem>
            <DropdownMenu>
                <DropdownMenuTrigger asChild>
                    <SidebarMenuButton
                        size="lg"
                        className={cn("data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground", {"md:h-8 md:p-0": collapsed})}
                    >
                        <UserBadge name={name} email={email} show_details={!collapsed}/>
                        <ChevronsUpDown className="ml-auto size-4" />
                    </SidebarMenuButton>
                </DropdownMenuTrigger>
                <DropdownMenuContent
                    className="w-[--radix-dropdown-menu-trigger-width] min-w-56 rounded-lg"
                    side={isMobile ? "bottom" : "right"}
                    align="end"
                    sideOffset={4}
                >
                    {collapsed ?
                        <DropdownMenuLabel className="p-0 font-normal">
                            <div className="flex items-center gap-2 px-1 py-1.5 text-left text-sm">
                                <UserBadge name={name} email={email} show_details={true}/>
                            </div>
                        </DropdownMenuLabel>
                        :
                        null
                    }
                    <Link to="/settings">
                        <DropdownMenuItem>Settings</DropdownMenuItem>
                    </Link>
                    <DropdownMenuItem onSelect={() => {
                        toggle_theme();
                    }}>
                        Switch Theme
                    </DropdownMenuItem>
                    <DropdownMenuSeparator/>
                    <DropdownMenuItem onSelect={(ev) => {
                        send_logout().then(() => {
                            navigate("/login");
                        }).catch(err => {
                            console.error("failed to logout:", err);
                        });
                    }}>
                        <LogOut />
                        Log out
                    </DropdownMenuItem>
                </DropdownMenuContent>
            </DropdownMenu>
        </SidebarMenuItem>
    </SidebarMenu>;
}

interface NavSidebarProps {
    collapsed: boolean
}

function NavSidebar({collapsed}: NavSidebarProps) {
    const {open, setOpen} = useSidebar();
    const location = useLocation();

    return <Sidebar
        collapsible="none"
        className={cn({"!w-[calc(var(--sidebar-width-icon)_+_1px)] border-r": collapsed})}
    >
        <SidebarHeader className={cn("flex gap-2", {"flex-row items-center": !collapsed && open, "flex-col": collapsed || !open})}>
            <UserMenu collapsed={collapsed || !open}/>
            <Button
                type="button"
                variant="ghost"
                size="icon"
                className={cn("flex-none", {"w-8 h-8": collapsed || !open})}
                onClick={() => setOpen(!open)}>
                {open ? <PanelLeftClose/> : <PanelLeft/>}
            </Button>
        </SidebarHeader>
        <SidebarContent>
            <SidebarGroup>
                <SidebarGroupContent className="px-1.5 md:px-0">
                    <SidebarMenu>
                        <SidebarMenuLink
                            title="Journals"
                            path="/journals"
                            active={location.pathname.startsWith("/journals")}
                            icon={<Notebook />}
                        />
                        <SidebarMenuLink
                            title="Administrative"
                            path="/admin"
                            active={location.pathname.startsWith("/admin")}
                            icon={<EarthLock />}
                        />
                        <SidebarMenuLink
                            title="Settings"
                            path="/settings"
                            active={location.pathname.startsWith("/settings")}
                            icon={<Settings />}
                        />
                    </SidebarMenu>
                </SidebarGroupContent>
            </SidebarGroup>
        </SidebarContent>
    </Sidebar>
}

interface MountStatusProps {
    element: ReactElement,
    mounted: () => void,
    unmounted: () => void,
}

function MountHook({element, mounted, unmounted}: MountStatusProps) {
    useEffect(() => {
        mounted();

        return () => {
            unmounted();
        }
    }, []);

    return element;
}

export function AppSidebar() {
    const [collapse, set_collapse] = useState(0b0);

    return <Sidebar
        collapsible="icon"
        className="overflow-hidden [&>[data-sidebar=sidebar]]:flex-row"
    >
        <NavSidebar collapsed={collapse > 0}/>
        <Sidebar
            collapsible="none"
            className="hidden flex-1 md:flex !w-[calc(var(--sidebar-width)_-_(var(--sidebar-width-icon)_+_1px))]"
        >
            <Routes>
                <Route path="/journals/:journals_id/*" element={<MountHook
                    mounted={() => set_collapse(v => v | 0b1)} 
                    unmounted={() => set_collapse(v => v & ~0b1)}
                    element={<JournalSidebar />}
                />}/>
                <Route path="/admin/*" element={<MountHook
                    mounted={() => set_collapse(v => v | 0b10)}
                    unmounted={() => set_collapse(v => v & ~0b10)}
                    element={<AdminSidebar />}
                />}/>
                <Route path="/settings/*" element={<MountHook
                    mounted={() => set_collapse(v => v | 0b100)}
                    unmounted={() => set_collapse(v => v & ~0b100)}
                    element={<SettingsSidebar />}
                />}/>
            </Routes>
        </Sidebar>
    </Sidebar>
}
