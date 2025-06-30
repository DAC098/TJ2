import { Link, Routes, Route, useNavigate, useLocation } from "react-router-dom";
import {
    ChevronsUpDown,
    LogOut,
    Notebook,
    Settings,
    EarthLock,
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
}

function UserBadge({name, email, avatar}: UserBadgeProps) {
    return <>
        <Avatar className="h-8 w-8 rounded-lg">
            {avatar != null ? <AvatarImage src={avatar} alt={name} /> : null}
            <AvatarFallback className="rounded-lg">TJ2</AvatarFallback>
        </Avatar>
        <div className="grid flex-1 text-left text-sm leading-tight">
            <span className="truncate font-semibold">{name}</span>
            <span className="truncate text-xs">{email}</span>
        </div>
    </>;
}

interface UserMenuProps {
    name: string,
    email: string,
    avatar?: string,
}

function UserMenu({name, email, avatar}: UserMenuProps) {
    const { isMobile } = useSidebar();
    const navigate = useNavigate();

    return <SidebarMenu>
        <SidebarMenuItem>
            <DropdownMenu>
                <DropdownMenuTrigger asChild>
                    <SidebarMenuButton
                        size="lg"
                        className="data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground md:h-8 md:p-0"
                    >
                        <UserBadge name={name} email={email} avatar={avatar}/>
                        <ChevronsUpDown className="ml-auto size-4" />
                    </SidebarMenuButton>
                </DropdownMenuTrigger>
                <DropdownMenuContent
                    className="w-[--radix-dropdown-menu-trigger-width] min-w-56 rounded-lg"
                    side={isMobile ? "bottom" : "right"}
                    align="end"
                    sideOffset={4}
                >
                    <DropdownMenuLabel className="p-0 font-normal">
                        <div className="flex items-center gap-2 px-1 py-1.5 text-left text-sm">
                            <UserBadge name={name} email={email} avatar={avatar}/>
                        </div>
                    </DropdownMenuLabel>
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
                    <DropdownMenuSeparator/>
                    <Link to="/settings">
                        <DropdownMenuItem>Settings</DropdownMenuItem>
                    </Link>
                    <DropdownMenuItem onSelect={() => {
                        toggle_theme();
                    }}>
                        Switch Theme
                    </DropdownMenuItem>
                </DropdownMenuContent>
            </DropdownMenu>
        </SidebarMenuItem>
    </SidebarMenu>;
}

interface NavSidebarProps {
    name: string,
    email: string,
    avatar?: string,
}

function NavSidebar({name, email, avatar}: NavSidebarProps) {
    const location = useLocation();

    return <Sidebar
        collapsible="none"
        className="!w-[calc(var(--sidebar-width-icon)_+_1px)] border-r"
    >
        <SidebarHeader>
            <UserMenu name={name} email={email} avatar={avatar}/>
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

export function AppSidebar() {
    return <Sidebar
        collapsible="icon"
        className="overflow-hidden [&>[data-sidebar=sidebar]]:flex-row"
    >
        <NavSidebar name="The Dude" email="the_dude@laboski.drink"/>
        <Sidebar
            collapsible="none"
            className="hidden flex-1 md:flex !w-[calc(var(--sidebar-width)_-_(var(--sidebar-width-icon)_+_1px))]"
        >
            <Routes>
                <Route path="/journals/:journals_id/*" element={<JournalSidebar />}/>
                <Route path="/admin/*" element={<AdminSidebar />}/>
                <Route path="/settings/*" element={<SettingsSidebar />}/>
            </Routes>
        </Sidebar>
    </Sidebar>
}
