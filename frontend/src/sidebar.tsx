import { Link, useNavigate } from "react-router-dom";
import {
    BadgeCheck,
    Bell,
    ChevronsUpDown,
    CreditCard,
    LogOut,
    Sparkles,
} from "lucide-react";

import {
    Avatar,
    AvatarFallback,
    AvatarImage,
} from "@/components/ui/avatar";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuGroup,
    DropdownMenuItem,
    DropdownMenuLabel,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
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


async function send_logout() {
    let res = await fetch("/logout", {
        method: "POST"
    });

    if (res.status != 200) {
        throw new Error("failed to logout user");
    }
}

interface MenuOptionProps {
    title: string,
    path: string
}

const MenuOption = ({title, path}: MenuOptionProps) => {
    return <SidebarMenuItem>
        <SidebarMenuButton asChild>
            <Link to={path}>{title}</Link>
        </SidebarMenuButton>
    </SidebarMenuItem>
};

const JournalGroup = () => {
    return <SidebarGroup>
        <SidebarGroupLabel>Journals</SidebarGroupLabel>
        <SidebarGroupContent>
            <SidebarMenu>
                <MenuOption title="Default" path="/entries"/>
            </SidebarMenu>
        </SidebarGroupContent>
    </SidebarGroup>
};

const AdministrativeGroup = () => {
    return <SidebarGroup>
        <SidebarGroupLabel>Administrative</SidebarGroupLabel>
        <SidebarGroupContent>
            <SidebarMenu>
                <MenuOption title="Users" path="/users"/>
            </SidebarMenu>
        </SidebarGroupContent>
    </SidebarGroup>
};

interface UserFooterProps {
    name: string,
    email: string,
    avatar: string,
}

const UserFooter = ({name, email, avatar,}: UserFooterProps) => {
    const { isMobile } = useSidebar();
    const navigate = useNavigate();

    return <SidebarMenu>
        <SidebarMenuItem>
            <DropdownMenu>
                <DropdownMenuTrigger asChild>
                    <SidebarMenuButton
                        size="lg"
                        className="data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground"
                    >
                        <Avatar className="h-8 w-8 rounded-lg">
                            <AvatarImage src={avatar} alt={name} />
                            <AvatarFallback className="rounded-lg">CN</AvatarFallback>
                        </Avatar>
                        <div className="grid flex-1 text-left text-sm leading-tight">
                            <span className="truncate font-semibold">{name}</span>
                            <span className="truncate text-xs">{email}</span>
                        </div>
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
                            <Avatar className="h-8 w-8 rounded-lg">
                                <AvatarImage src={avatar} alt={name} />
                                <AvatarFallback className="rounded-lg">CN</AvatarFallback>
                            </Avatar>
                            <div className="grid flex-1 text-left text-sm leading-tight">
                                <span className="truncate font-semibold">{name}</span>
                                <span className="truncate text-xs">{email}</span>
                            </div>
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
                </DropdownMenuContent>
            </DropdownMenu>
        </SidebarMenuItem>
    </SidebarMenu>
}

const AppSidebar = () => {
    return <Sidebar>
        <SidebarContent>
            <JournalGroup />
            <AdministrativeGroup />
        </SidebarContent>
        <SidebarFooter>
            <UserFooter name="The Dude" email="the_dude@laboski.drink" avatar="/noop"/>
        </SidebarFooter>
    </Sidebar>
};

export { AppSidebar };
