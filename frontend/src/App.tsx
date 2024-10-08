import { BrowserRouter, Routes, Route, Link, useNavigate } from "react-router-dom";

import Entries from "./Entries";
import Entry from "./Entry";

async function send_logout() {
    let res = await fetch("/logout", {
        method: "POST"
    });

    if (res.status != 200) {
        throw new Error("failed to logout user");
    }
}

const App = () => {
    const navigate = useNavigate();

    return <div className="flex flex-row flex-nowrap w-full h-full">
        <nav className="flex-none w-40">
            <div>
                <button onClick={() => {
                    send_logout().then(() => {
                        navigate("/login");
                    }).catch(err => {
                        console.error("failed to logout:", err);
                    });
                }}>
                    Logout
                </button>
            </div>
            <div>
                <Link to="/entries/new">New Entry</Link>
            </div>
            <div>
                <Link to="/entries">Entries</Link>
            </div>
        </nav>
        <main className="relative flex-auto overflow-scroll">
            <Routes>
                <Route path="/" element={<div>root page</div>}/>
                <Route path="/entries" element={<Entries />}/>
                <Route path="/entries/:entry_date" element={<Entry />}/>
            </Routes>
        </main>
    </div>
};

export default App;
