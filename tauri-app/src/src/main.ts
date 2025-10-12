import './map';

const genBtn = document.getElementById('gen') as HTMLButtonElement;
const bboxDiv = document.getElementById('bbox') as HTMLDivElement;
let currentBBox: any = null;

async function generate() {
  if(!currentBBox) return alert('Select area');
  genBtn.disabled = true;
  // @ts-ignore
  const res = await window.__TAURI__.invoke('generate_map', { south: currentBBox.south, west: currentBBox.west, north: currentBBox.north, east: currentBBox.east });
  alert('Generated at ' + res);
  genBtn.disabled = false;
}

(genBtn).addEventListener('click', generate);

// listen progress events
// @ts-ignore
window.__TAURI__.event.listen('bng_progress', event => {
  const payload = event.payload as string;
  try {
    const obj = JSON.parse(payload);
    const p = obj.progress || 0;
    const text = obj.text || '';
    const bar = document.getElementById('progressbar');
    if(bar) bar.innerText = `${p}% - ${text}`;
  } catch(e) {}
});

// map.ts will call window.setBBox(bbox)
// @ts-ignore
window.setBBox = (bbox:any)=>{ currentBBox = bbox; bboxDiv.innerText = JSON.stringify(bbox); };
