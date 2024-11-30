import { useState, useEffect } from "react";
import { Link } from "react-router-dom";
import { Plus } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
    DataTable,
    ColumnDef,
} from "@/components/ui/table";

interface UserPartial {
    id: number,
    uid: string,
    username: string,
    created: string,
    updated: string | null
}

async function get_users() {
    let res = await fetch("/users");

    if (res.status !== 200) {
        console.log("non 200 response status:", res);

        return null;
    }

    return await res.json() as UserPartial[];
}

const columns: ColumnDef<UserPartial>[] = [
    {
        accessorKey: "username",
        header: "Username",
        cell: ({ row }) => {
            return <Link to={`/users/${row.original.id}`}>{row.original.username}</Link>;
        }
    },
    {
        accessorKey: "mod",
        header: "Mod",
        cell: ({ row }) => {
            return row.original.updated != null ? row.original.updated : row.original.created;
        }
    }
];

export function Users() {
    let [loading, set_loading] = useState(false);
    let [users, set_users] = useState<UserPartial[]>([]);

    useEffect(() => {
        set_loading(true);

        get_users().then(list => {
            if (list == null) {
                return;
            }

            set_users(list);
        }).catch(err => {
            console.log("failed to load user list");
        }).finally(() => {
            set_loading(false);
        });
    }, []);

    return <div className="max-w-3xl mx-auto my-auto space-y-4">
        <div className="flex flex-row flex-nowrap gap-x-4">
            <Link to="/users/new">
                <Button type="button">New User<Plus/></Button>
            </Link>
        </div>
        <DataTable columns={columns} data={users}/>
    </div>;
};
