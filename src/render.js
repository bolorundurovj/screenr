/*
 *   Copyright (c) 2020
 *   All rights reserved.
 */

const { desktopCapturer, remote } = require('electron');
const { writeFile } = require('fs');
const { Menu, dialog } = remote;

//Buttons
const videoElement = document.querySelector('video');
const startBtn = document.getElementById('startBtn');
const stopBtn = document.getElementById('stopBtn');
const videoSelectBtn = document.getElementById('videoSelectBtn');
const pageTitle = document.getElementById('title');

//Get all available video sources
videoSelectBtn.onclick = async function getVideoSources() {
  const inputSources = await desktopCapturer.getSources({
    types: ['window', 'screen'],
  });

  const videoOptionsMenu = Menu.buildFromTemplate(
    inputSources.map((source) => {
      return {
        label: source.name,
        click: () => selectSource(source),
      };
    })
  );

  videoOptionsMenu.popup();
}

startBtn.onclick = (e) => {
  mediaRecorder.start();
  startBtn.classList.add('is-danger');
  startBtn.innerText = 'Recording';
};

stopBtn.onclick = (e) => {
  mediaRecorder.stop();
  startBtn.classList.remove('is-danger');
  startBtn.innerText = 'Start';
};

let mediaRecorder; //Mediarecorder instance to capture footage
const recordedChunks = [];

async function selectSource(source) {
  pageTitle.innerText = source.name;
  videoSelectBtn.innerText = 'Change Video Source';

  const constraints = {
    audio: false,
    video: {
      mandatory: {
        chromeMediaSource: 'desktop',
        chromeMediaSourceId: source.id,
      },
    },
  };

  //Create a stream
  const stream = await navigator.mediaDevices.getUserMedia(constraints);

  //Preview the source in a video element
  videoElement.srcObject = stream;
  videoElement.play();

  //Create the Media Recorder
  const options = { mimeType: 'video/webm; codecs=vp9' };
  mediaRecorder = new MediaRecorder(stream, options);

  //Register Event Handlers
  mediaRecorder.ondataavailable = handleAvailableData;
  mediaRecorder.onstop = handleStop;
}

async function handleAvailableData(e) {
  recordedChunks.push(e.data);
}

//Save video on stop
async function handleStop(e) {
  const blob = new Blob(recordedChunks, {
    type: 'video/webm; codecs=vp9',
  });

  const buffer = Buffer.from(await blob.arrayBuffer());

  const { filePath } = await dialog.showSaveDialog({
    buttonLabel: 'Save Video',
    defaultPath: `vid-${Date.now()}.webm`,
  });

  if (filePath) {
    writeFile(filePath, buffer, () => console.log('Saved Successfully'));
  }
}
