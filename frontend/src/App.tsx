import { BrowserRouter, Routes, Route, Link } from "react-router-dom";

import Entries from "./Entries";
import Entry from "./Entry";

const App = () => {
    return <div className="flex flex-row flex-nowrap w-full h-full">
        <nav className="flex-none w-40">
            <div>
                <button
                    className=""
                    onClick={() => {
                        console.log("request logout");
                    }}
                >
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
