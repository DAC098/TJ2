import { useState, useEffect } from "react";
import { Link } from "react-router-dom";

import { ViewDate } from "./components/time";

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

const Users = () => {
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

    let user_rows = [];

    for (let user of users) {
        let date = new Date(user.updated != null ? user.updated : user.created);

        user_rows.push(<tr key={user.id}>
            <td>
                <Link to={`/users/${user.id}`}>{user.username}</Link>
            </td>
            <td><ViewDate date={date}/></td>
        </tr>);
    }

    if (loading) {
        return <div>loading users</div>;
    } else {
        return <div>
            <table>
                <thead>
                    <tr className="sticky top-0 bg-white">
                        <th>Username</th>
                        <th>Mod</th>
                    </tr>
                </thead>
                <tbody>{user_rows}</tbody>
            </table>
        </div>;
    }
};

export default Users;
