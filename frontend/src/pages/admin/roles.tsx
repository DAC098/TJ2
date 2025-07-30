import { Plus } from "lucide-react";
import { useState, useEffect } from "react";
import { Link } from "react-router-dom";

import { Button } from "@/components/ui/button";
import {
    DataTable,
    ColumnDef,
} from "@/components/ui/table";

export interface RolePartial {
    id: number,
    uid: string,
    name: string,
    created: string,
    updated: string | null
}

async function get_roles() {
    let res = await fetch("/admin/roles");

    if (res.status !== 200) {
        console.log("non 200 response status:", res);

        return null;
    }

    return await res.json() as RolePartial[];
}

export function Roles() {
    let [loading, set_loading] = useState(false);
    let [roles, set_roles] = useState<RolePartial[]>([]);

    const columns: ColumnDef<RolePartial>[] = [
        {
            header: "Name",
            cell: ({ row }) => {
                return <Link to={`/admin/roles/${row.original.id}`}>{row.original.name}</Link>;
            },
        },
        {
            header: "Mod",
            cell: ({ row }) => {
                return row.original.updated != null ? row.original.updated : row.original.created;
            },
        },
    ];

    useEffect(() => {
        set_loading(true);

        get_roles().then(list => {
            if (list == null) {
                return;
            }

            set_roles(list);
        }).catch(err => {
            console.log("failed to load roles list", err);
        }).finally(() => {
            set_loading(false);
        });
    }, []);

    return <div className="max-w-3xl mx-auto my-auto space-y-4">
        <div className="flex flex-row flex-nowrap gap-x-4">
            <Link to="/admin/roles/new">
                <Button type="button">New Role<Plus/></Button>
            </Link>
        </div>
        <DataTable columns={columns} data={roles}/>
    </div>;
}
