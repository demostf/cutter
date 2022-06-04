import {cut} from "democutter";


let fileSelect = document.getElementById('file');
let startInput = document.getElementById('start');
let endInput = document.getElementById('end');
let cutButton = document.getElementById('cut');

let outputName = "cut.dem";

cutButton.addEventListener('click', (event) => {
    let start = parseInt(startInput.value);
    let end = parseInt(endInput.value);
    console.log(start, end);
    fileSelect.disabled = true;
    let reader = new FileReader();
    reader.readAsArrayBuffer(fileSelect.files[0]);
    reader.addEventListener('load', () => {
        console.log(reader.result);
        let result = cut(new Uint8Array(reader.result), start, end);
        fileSelect.disabled = false;
        save(result, outputName);
    });
});


fileSelect.addEventListener('change', (event) => {
    const tickRate = 66;
    let name = fileSelect.files[0].name;
    let match = name.match(/^([^_]+)_(\d+)\.dem$/);
    if (match) {
        let highlightTick = parseInt(match[2]);
        startInput.value = highlightTick;
        endInput.value = highlightTick + tickRate * 5 * 60;
        outputName = `${match[1]}_${tickRate * 10}.dem`;
    } else {
        outputName = name.replace(/\.dem/, "_cut.dem");
    }
});

function save(data, fileName) {
    let a = document.createElement("a");
    document.body.appendChild(a);
    a.style = "display: none";
    let blob = new Blob([data], {type: "octet/stream"});
    let url = window.URL.createObjectURL(blob);
    a.href = url;
    a.download = fileName;
    a.click();
    window.URL.revokeObjectURL(url);
}
