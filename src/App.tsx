// src/App.tsx
import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/api/dialog';
import { MapContainer, TileLayer, Rectangle, useMapEvents } from 'react-leaflet';
import 'leaflet/dist/leaflet.css';
import './App.css';

interface BoundingBox {
  min_lat: number;
  min_lng: number;
  max_lat: number;
  max_lng: number;
}

interface GenerationProgress {
  stage: string;
  progress: number;
}

function MapSelector({ onBoundsChange }: { onBoundsChange: (bounds: BoundingBox) => void }) {
  const [selectionStart, setSelectionStart] = useState<[number, number] | null>(null);
  const [selectionEnd, setSelectionEnd] = useState<[number, number] | null>(null);

  const MapEvents = () => {
    useMapEvents({
      click: (e) => {
        if (!selectionStart) {
          setSelectionStart([e.latlng.lat, e.latlng.lng]);
        } else if (!selectionEnd) {
          setSelectionEnd([e.latlng.lat, e.latlng.lng]);
          
          const bbox: BoundingBox = {
            min_lat: Math.min(selectionStart[0], e.latlng.lat),
            min_lng: Math.min(selectionStart[1], e.latlng.lng),
            max_lat: Math.max(selectionStart[0], e.latlng.lat),
            max_lng: Math.max(selectionStart[1], e.latlng.lng),
          };
          onBoundsChange(bbox);
        }
      },
    });
    return null;
  };

  const resetSelection = () => {
    setSelectionStart(null);
    setSelectionEnd(null);
  };

  return (
    <div className="map-container">
      <MapContainer
        center={[51.505, -0.09]}
        zoom={13}
        style={{ height: '500px', width: '100%' }}
      >
        <TileLayer
          attribution='&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a>'
          url="https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png"
        />
        <MapEvents />
        
        {selectionStart && selectionEnd && (
          <Rectangle
            bounds={[selectionStart, selectionEnd]}
            pathOptions={{ color: 'blue', fillOpacity: 0.2 }}
          />
        )}
      </MapContainer>
      
      <div className="selection-info">
        {selectionStart && !selectionEnd && (
          <p>Кликните второй раз, чтобы завершить выбор области</p>
        )}
        {selectionStart && selectionEnd && (
          <button onClick={resetSelection}>Сбросить выбор</button>
        )}
      </div>
    </div>
  );
}

function App() {
  const [bbox, setBbox] = useState<BoundingBox | null>(null);
  const [outputPath, setOutputPath] = useState<string>('');
  const [isGenerating, setIsGenerating] = useState(false);
  const [progress, setProgress] = useState<GenerationProgress>({
    stage: '',
    progress: 0,
  });
  const [result, setResult] = useState<string>('');

  useEffect(() => {
    const unlisten = listen<GenerationProgress>('generation-progress', (event) => {
      setProgress(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const selectOutputPath = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: 'Выберите папку для сохранения карты',
    });

    if (selected && typeof selected === 'string') {
      setOutputPath(selected);
    }
  };

  const generateTerrain = async () => {
    if (!bbox || !outputPath) {
      alert('Пожалуйста, выберите область на карте и путь для сохранения');
      return;
    }

    setIsGenerating(true);
    setResult('');

    try {
      const response = await invoke<string>('generate_terrain', {
        bbox,
        outputPath,
      });
      setResult(response);
    } catch (error) {
      setResult(`Ошибка: ${error}`);
    } finally {
      setIsGenerating(false);
    }
  };

  return (
    <div className="App">
      <header className="App-header">
        <h1>🚗 BeamNG Terrain Generator</h1>
        <p>Генератор карт BeamNG.drive из реальных локаций</p>
      </header>

      <main className="App-main">
        <section className="map-section">
          <h2>1. Выберите область на карте</h2>
          <p>Кликните дважды на карте, чтобы выбрать прямоугольную область</p>
          <MapSelector onBoundsChange={setBbox} />
          
          {bbox && (
            <div className="bbox-info">
              <h3>Выбранная область:</h3>
              <p>Широта: {bbox.min_lat.toFixed(6)} - {bbox.max_lat.toFixed(6)}</p>
              <p>Долгота: {bbox.min_lng.toFixed(6)} - {bbox.max_lng.toFixed(6)}</p>
            </div>
          )}
        </section>

        <section className="output-section">
          <h2>2. Выберите путь для сохранения</h2>
          <button onClick={selectOutputPath} className="select-button">
            📁 Выбрать папку
          </button>
          {outputPath && <p className="path-display">Путь: {outputPath}</p>}
        </section>

        <section className="generate-section">
          <h2>3. Генерация карты</h2>
          <button
            onClick={generateTerrain}
            disabled={!bbox || !outputPath || isGenerating}
            className="generate-button"
          >
            {isGenerating ? '⏳ Генерация...' : '🚀 Генерировать карту'}
          </button>

          {isGenerating && (
            <div className="progress-container">
              <div className="progress-bar">
                <div
                  className="progress-fill"
                  style={{ width: `${progress.progress}%` }}
                />
              </div>
              <p className="progress-text">
                {progress.stage} - {progress.progress.toFixed(0)}%
              </p>
            </div>
          )}

          {result && (
            <div className={`result ${result.includes('Ошибка') ? 'error' : 'success'}`}>
              {result}
              {!result.includes('Ошибка') && (
                <div className="mod-info">
                  <h3>✅ Мод успешно создан!</h3>
                  <p>📦 Файл мода: <code>generated_map.zip</code></p>
                  <h4>Как установить:</h4>
                  <ol>
                    <li>Скопируйте <code>generated_map.zip</code> в папку модов BeamNG</li>
                    <li>Windows: <code>%USERPROFILE%\AppData\Local\BeamNG.drive\0.32\mods\</code></li>
                    <li>Linux: <code>~/.local/share/BeamNG.drive/0.32/mods/</code></li>
                    <li>Запустите BeamNG.drive</li>
                    <li>Активируйте мод в Repository</li>
                    <li>Выберите уровень "Generated Map" в меню уровней</li>
                  </ol>
                </div>
              )}
            </div>
          )}
        </section>

        <section className="info-section">
          <h2>ℹ️ Информация</h2>
          <ul>
            <li>✅ Terrain данные загружаются из AWS Terrain Tiles</li>
            <li>✅ Объекты (здания, деревья, остановки) из OpenStreetMap</li>
            <li>✅ Полная дорожная сеть с road_nodes</li>
            <li>✅ Автоматическая конвертация в формат BeamNG.drive</li>
            <li>✅ Создаётся готовый ZIP мод для установки</li>
            <li>✅ Поддержка больших областей</li>
          </ul>
          
          <div className="mod-structure">
            <h3>📦 Структура мода:</h3>
            <pre>
generated_map.zip
├── info.json (метаданные мода)
├── levels/
│   └── generated_map/
│       ├── main.level.json (конфиг уровня)
│       ├── items.level.json (объекты)
│       ├── road_nodes.json (дорожная сеть)
│       ├── decalRoad.json (дороги BeamNG)
│       ├── preview.jpg (превью карты)
│       └── art/
│           └── terrains/
│               ├── terrain.png (heightmap)
│               └── terrain.ter.json (настройки)
            </pre>
          </div>
        </section>
      </main>

      <footer className="App-footer">
        <p>Made with ❤️ using Rust + Tauri</p>
      </footer>
    </div>
  );
}

export default App;