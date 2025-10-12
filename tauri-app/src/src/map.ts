import L from 'leaflet';
import 'leaflet-draw';

const map = L.map('map').setView([55.76,37.64], 12);
L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png',{maxZoom:19}).addTo(map);
const drawnItems = new L.FeatureGroup(); map.addLayer(drawnItems);
const drawControl = new L.Control.Draw({draw:{rectangle:true, polygon:false, circle:false, polyline:false, marker:false}, edit:{featureGroup:drawnItems}});
map.addControl(drawControl);
let lastBounds: L.LatLngBounds | null = null;
map.on((L as any).Draw.Event.CREATED, function(e:any){ drawnItems.clearLayers(); drawnItems.addLayer(e.layer); lastBounds = e.layer.getBounds(); });

const sendBtn = document.getElementById('send') as HTMLButtonElement;
sendBtn.addEventListener('click', ()=>{
  if(!lastBounds) return alert('Draw rectangle');
  const bbox = {south: lastBounds.getSouth(), west: lastBounds.getWest(), north: lastBounds.getNorth(), east: lastBounds.getEast()};
  // @ts-ignore
  window.setBBox(bbox);
});
