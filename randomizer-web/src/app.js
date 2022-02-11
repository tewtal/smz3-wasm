import React, { useRef } from 'react';
import { ConsoleInterface } from 'console-interface';

function App(props) {
    const sni_interface = useRef(new ConsoleInterface("sni"));
    const usb_interface = useRef(new ConsoleInterface("usb2snes"));

    const readMemoryTest = async (ci) => {
        const devices = await ci.list_devices();
        console.log(devices);
        console.log("Read", ci, await ci.read_multi(devices[0].uri, [0, 0x500]));
    }

    return <>
        <h1>Hello World</h1>
        <button onClick={async () => await readMemoryTest(sni_interface.current)}>Test SNI</button>
        <button onClick={async () => await readMemoryTest(usb_interface.current)}>Test WebSocket</button>
    </>;
}

export default App;