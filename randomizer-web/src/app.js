import React, { useCallback, useRef } from 'react';
import { get_session, Message } from 'randomizer-client';
import { ConsoleInterface } from 'console-interface';

function App(props) {
    const console_interface = useRef(new ConsoleInterface("USB2SNES"));


    const readMemoryTest = async (proto) => {
        //const ci = new ConsoleInterface(proto);
        //await ci.connect();
        const ci = console_interface.current;
        const devices = await ci.list_devices();
        let data = await ci.read_memory(devices[0].uri, 0, 1024);
        let more_data = await ci.read_memory(devices[0].uri, 1024, 1024);
        return data;        
    }

    return <>
        <h1>Hello World</h1>
        <button onClick={async () => console.log(await readMemoryTest("SNI"))}>Test SNI</button>
        <button onClick={async () => console.log(await readMemoryTest("USB2SNES"))}>Test WebSocket</button>
    </>;
}

export default App;