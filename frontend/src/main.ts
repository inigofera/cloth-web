const root = document.getElementById('root');

if (root) {
  const heading = document.createElement('h1');
  heading.textContent = 'Cloth Web Frontend';

  const info = document.createElement('p');
  info.textContent = 'Vite + TypeScript is running.';

  root.appendChild(heading);
  root.appendChild(info);
}


