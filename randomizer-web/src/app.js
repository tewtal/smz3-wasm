import React, { useRef } from 'react';
import { ConsoleInterface } from 'randomizer-client';

function App(props) {
    const sni_interface = useRef(new ConsoleInterface("sni"));
    const usb_interface = useRef(new ConsoleInterface("usb2snes"));

    const readMemoryTest = async (ci) => {
        const devices = await ci.list_devices();
        console.log(devices);
        for(let i = 0; i < 500; i++) {
            //console.log("Write", i, await ci.write_multi(devices[0].uri, [0, 0x10], [new Uint8Array([0,1,2,3,4,5,6,7,8,9]), new Uint8Array([0,1,2,3,4,5,6,7,8,9])]));
            console.log("Write", i, await ci.write(devices[0].uri, 0, new Uint8Array([0,1,2,3,4,5,6,7,8,9])));
        }
        
        //console.log("Read", ci, await ci.read_multi(devices[0].uri, [0, 0x20]));
    }

    return <>
        <h1>Hello World</h1>
        <button onClick={async () => await readMemoryTest(sni_interface.current)}>Test SNI</button>
        <button onClick={async () => await readMemoryTest(usb_interface.current)}>Test WebSocket</button>
    </>;
}

export default App;