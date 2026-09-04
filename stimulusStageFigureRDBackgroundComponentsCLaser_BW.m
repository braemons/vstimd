function output = stimulusStageFigureRDBackgroundComponentsCLaser_BW(stage,screenInfo,input)

if strcmp(stage,'init')
    params = [];

    params{1}.Ndots = 80;
    params{1}.dotSpacing = 5;
    params{1}.dotSize = 1.5; % degrees
    params{1}.dotJitter = 200;
    params{1}.cohPropBackground = [1];
    params{1}.cohPropFigure = [1];
    params{1}.vel = 50; %degrees per sec
    %     params{1}.dirAngleBackground = [0 pi/2];
    params{1}.dirAngleBackground = [0, 3*pi/2];
    params{1}.dirAngleFigure = [0, 3*pi/2];%2 DIRS
%     params{1}.dirAngleFigure = [0:pi/2:(2*pi-pi/2)];%[0 pi/2 pi 3*pi/2];%4 DIRS
    %     params{1}.dirAngleFigure = [0:pi/4:(2*pi-pi/4)];%[0 pi/2 pi 3*pi/2]%8 DIRS

    params{1}.figureDotIntensity = 255;
    params{1}.bwSameTrial = [0]; %0 - all white, 1 - black and white

    params{1}.backgroundDotIntensity = 255;
    params{1}.visFrames = [120];
    params{1}.preVisFrames = [60];
    params{1}.noFigureFrames = 0;

    params{1}.backgroundCol = 128;
    %params{1}.backgroundCol = 0;


    %params{1}.xc = [1];
    %params{1}.yc = [1];
    params{1}.xc = screenInfo.RFcenter.x;
    params{1}.yc = screenInfo.RFcenter.y;
   % params{1}.R = [20 45]/2;
%     params{1}.R = [10 20 45 60]/2;
     params{1}.R = [45]/2;
    params{1}.inverted0classic1 = [1];

% 
%     params{1}.R0G1B2 = [0];% sin 0 to laser amp, Bipoles 0 - red, ChR2 1 - Blue laser
%     params{1}.RedlaserAmpl = unique(screenInfo.RedLaserIntensity_mW);
%     params{1}.BluelaserAmpl = 0;
%     params{1}.GreenlaserAmpl = 0;

    %     params{1}.R0G1B2 = [1];% sin 0 to laser amp, Bipoles 0 - red, ChR2 1 - Blue laser
    %     params{1}.RedlaserAmpl = 0;
    %     params{1}.BluelaserAmpl = 0;
    %     params{1}.GreenlaserAmpl = unique(screenInfo.GreenLaserIntensity_mW);
    %
    params{1}.R0G1B2 = [0,2];% sin 0 to laser amp, Bipoles 0 - red, ChR2 1 - Blue laser
    params{1}.RedlaserAmpl = 0;
    params{1}.BluelaserAmpl = unique(screenInfo.BlueLaserIntensity_mW);
    params{1}.GreenlaserAmpl = 0;


    params{1}.laserFreq = 0; %[0 30];
    params{1}.random = [0];

    params{1}.laserStart = 10000;
    params{1}.laserDur = 20000;
    params{1}.LaserStartPulse = 0.8*20000; %laser streaming starts 0.8s (800ms) before vis stim
    params{1}.preVisLaserFrames = 48; %60 per second, 48 per 800 ms
    params{1}.laserBufferCount = 20000+16000+20000;
    params{1}.laserRate = 20000;
    params{1}.ShortPdCO = [0];% for short pulse only blue and green
    params{1}.LongPdCO = [0];

    % figure only
    params{2} = params{1};
    params{2}.dirAngleBackground = [0];
    params{2}.dirAngleFigure = [0, 3*pi/2];%2 DIRS
%     params{2}.dirAngleFigure = [0:pi/2:(2*pi-pi/2)];%[0 pi/2 pi 3*pi/2];%4 DIRS
    params{2}.R = [10 20 30 45 60 220]/2;
    %     params{2}.R = [20 45]/2;
    params{2}.backgroundDotIntensity = 0;
% 
    % BG only - hole
    params{3} = params{1};
    params{3}.dirAngleBackground = [0, 3*pi/2];
    %     params{3}.dirAngleBackground = [0 pi/2];
    params{3}.dirAngleFigure = [0];%[0 pi/2 pi 3*pi/2];
    params{3}.R = [10 20 30 45 60]/2;
   % params{3}.R = [45]/2;
    params{3}.figureDotIntensity = 0;
    params{3}.backgroundDotIntensity = 255;

    params{4} = params{1};
    params{4}.dirAngleBackground = [0];
    params{4}.dirAngleFigure = [0];
    params{4}.figureDotIntensity = 0;
    params{4}.backgroundDotIntensity = 0;
    params{4}.R = [10]/2;

    % iso - coz params 1 has a fig ring!
%     params{5} = params{1};
%     params{5}.dirAngleBackground = [0];
%     params{5}.dirAngleFigure = [0, 3*pi/2];
%     params{5}.backgroundDotIntensity = 0;
%     params{5}.R = [220]/2;

%     params{2} = params{1};
%     params{2}.dirAngleBackground = [0];
%     params{2}.dirAngleFigure = [0];
%     params{2}.figureDotIntensity = 0;
%     params{2}.backgroundDotIntensity = 0;
%     params{2}.R = [20]/2;
% 
%     % iso - coz params 1 has a fig ring!
%     params{3} = params{1};
%     params{3}.dirAngleBackground = [0];
%     params{3}.dirAngleFigure = [0, 3*pi/2];
%     params{3}.backgroundDotIntensity = 0;
%     params{3}.R = [220]/2;

    output = params;

elseif strcmp(stage,'load')
    output = input;
    output.orgPosB = (rand(2,(input.Ndots*2+1)*(input.Ndots*2+1))-0.5)*2*input.dotJitter;
    output.dotIntensityB = round(rand(1,(input.Ndots*2+1)*(input.Ndots*2+1)))*255;
    if output.bwSameTrial == 0
        output.dotIntensityB(:) = 255;
    end

    angles = rand(1,(input.Ndots*2+1)*(input.Ndots*2+1))*2*3.141592;
    targetDir = repmat([cos(input.dirAngleBackground) ; sin(input.dirAngleBackground)],1,length(angles));
    if 0
        dirs = (1-input.propDir)*[cos(angles) ; sin(angles)]+input.propDir*targetDir;
        output.dirs = bsxfun(@times,dirs,1./sqrt(sum(dirs.^2,1)));
    else
        randDirs = [cos(angles) ; sin(angles)];

        rps = randperm(length(angles));
        nonRandInds = rps(1:round(length(angles)*input.cohPropBackground));
        randInds = setdiff(rps,nonRandInds);
        output.dirsB(:,randInds) = randDirs(:,randInds);
        output.dirsB(:,nonRandInds) = targetDir(:,nonRandInds);
    end

    output.orgPosF = (rand(2,(input.Ndots*2+1)*(input.Ndots*2+1))-0.5)*2*input.dotJitter;
    output.dotIntensityF = round(rand(1,(input.Ndots*2+1)*(input.Ndots*2+1)))*255;
    if output.bwSameTrial == 0
        output.dotIntensityF(:) = 255;
    end
    angles = rand(1,(input.Ndots*2+1)*(input.Ndots*2+1))*2*3.141592;
    targetDir = repmat([cos(input.dirAngleFigure) ; sin(input.dirAngleFigure)],1,length(angles));
    if 0
        dirs =(1-input.propDir)*[cos(angles) ; sin(angles)]+input.propDir*targetDir;
        output.dirs = bsxfun(@times,dirs,1./sqrt(sum(dirs.^2,1)));
    else
        randDirs = [cos(angles) ; sin(angles)];

        rps = randperm(length(angles));
        nonRandInds = rps(1:round(length(angles)*input.cohPropFigure));
        randInds = setdiff(rps,nonRandInds);
        output.dirsF(:,randInds) = randDirs(:,randInds);
        output.dirsF(:,nonRandInds) = targetDir(:,nonRandInds);
    end

    % ******************* Laser code **********************************

    output.laser = zeros(3,output.laserBufferCount)+1;

    if output.R0G1B2 == 0

        output.laser(1,:) = output.laser(1,:)*output.RedlaserAmpl;
        output.laser(2,:) = output.laser(2,:)*0;
        output.laser(3,:) = output.laser(3,:)*0;


        output.laser(1,1:output.laserStart) = 0;
        output.laser(1,(output.laserStart+output.laserDur):end) = 0;

        output.PrelaserVec = zeros(3,output.LaserStartPulse);

        output.laser = quickFilter(output.laser',10,1)';

        output.laser = cat(2,output.PrelaserVec,output.laser);

    elseif output.R0G1B2 == 1

        output.laser(1,:) = output.laser(1,:)*0;
        output.laser(2,:) = output.laser(2,:)*0;
        output.laser(3,:) = output.laser(3,:)*output.GreenlaserAmpl;


        output.laser(3,1:output.laserStart) = 0;
        output.laser(3,(output.laserStart+output.laserDur):end) = 0;

        output.PrelaserVec = zeros(3,output.LaserStartPulse);

        output.laser = quickFilter(output.laser',10,1)';

        output.laser = cat(2,output.PrelaserVec,output.laser);

    elseif output.R0G1B2 == 2

        output.laser(1,:) = output.laser(1,:)*0;
        output.laser(2,:) = output.laser(2,:)*output.BluelaserAmpl;
        output.laser(3,:) = output.laser(3,:)*0;


        output.laser(2,1:output.laserStart) = 0;
        output.laser(2,(output.laserStart+output.laserDur):end) = 0;

        output.PrelaserVec = zeros(3,output.LaserStartPulse);

        output.laser = quickFilter(output.laser',10,1)';

        output.laser = cat(2,output.PrelaserVec,output.laser);

    end

    if 0
        figure(1); clf; plot([dirs(1,:)*0 ; dirs(1,:)],[dirs(2,:)*0 ; dirs(2,:)]); axis equal; pause
    end


elseif strcmp(stage,'stim')
    stim = input;

    centerPos = [round(screenInfo.rect(3)/2) round(screenInfo.rect(4)/2)];

    % Grating generation
    pixelXc = (stim.xc*screenInfo.deg2pix)+screenInfo.StimResWidth/2;
    pixelYc = (stim.yc*screenInfo.deg2pix)+screenInfo.StimResHeight/2;

    xs = 1:screenInfo.StimResWidth;
    ys = 1:screenInfo.StimResHeight;
    figureMask = sqrt(bsxfun(@plus,(xs-pixelXc).^2,((ys-pixelYc)').^2));

    if stim.inverted0classic1 == 0
        figureMask = figureMask > (stim.R*screenInfo.deg2pix);
    else
        figureMask = figureMask < (stim.R*screenInfo.deg2pix);
    end

    im = zeros(screenInfo.StimResHeight,screenInfo.StimResWidth)+stim.backgroundCol;

    % Background Random dot generation
    stim.dotSpacing = stim.dotSpacing*screenInfo.deg2pix;
    stim.vel = stim.vel*screenInfo.deg2pix;
    stim.dotSize = round(stim.dotSize*screenInfo.deg2pix);

    h = screenInfo.StimResHeight;
    w = screenInfo.StimResWidth;

    radius = stim.dotSize;
    edges = (-radius):radius;
    dotMask = sqrt(bsxfun(@plus,edges.^2,(edges').^2)) < radius;

    yis = (-stim.Ndots):stim.Ndots;
    xis = (-stim.Ndots):stim.Ndots;
    yism = repmat(yis',1,length(yis))*stim.dotSpacing;
    xism = repmat(xis,length(xis),1)*stim.dotSpacing;

    xtot = centerPos(1)+xism(:)'+stim.orgPosB(1,:)+stim.frameIndex/screenInfo.FrameRate*stim.dirsB(1,:)*stim.vel;
    ytot = centerPos(2)+yism(:)'+stim.orgPosB(2,:)+stim.frameIndex/screenInfo.FrameRate*stim.dirsB(2,:)*stim.vel;

    inds = find((xtot > -stim.dotSize).*(xtot < w+stim.dotSize).*(ytot > -stim.dotSize).*(ytot < h+stim.dotSize));

    for i=1:length(inds)
        x = round(xtot(inds(i)));
        y = round(ytot(inds(i)));


        xs = x+edges;
        ys = y+edges;

        if (xs(1) <= 0) || (xs(end) > size(im,2)) || (ys(1) <= 0) || (ys(end) > size(im,1))
            xinds = find((xs > 0).*(xs <= size(im,2)));
            yinds = find((ys > 0).*(ys <= size(im,1)));

            %im(ys(yinds),xs(xinds)) = im(ys(yinds),xs(xinds)).*(1-dotMask(yinds,xinds)) + dotMask(yinds,xinds)*stim.col;
        else
            if figureMask(y,x)==0 && stim.backgroundDotIntensity == 255
                im(ys,xs) = im(ys,xs).*(1-dotMask) + dotMask*stim.dotIntensityB(inds(i));
            end
        end
    end

    % Figure ground

    h = screenInfo.StimResHeight;
    w = screenInfo.StimResWidth;

    radius = stim.dotSize;
    edges = (-radius):radius;
    dotMask = sqrt(bsxfun(@plus,edges.^2,(edges').^2)) < radius;

    yis = (-stim.Ndots):stim.Ndots;
    xis = (-stim.Ndots):stim.Ndots;
    yism = repmat(yis',1,length(yis))*stim.dotSpacing;
    xism = repmat(xis,length(xis),1)*stim.dotSpacing;

    if stim.frameIndex < stim.noFigureFrames
        xtot = centerPos(1)+xism(:)'+stim.orgPosB(1,:)+stim.frameIndex/screenInfo.FrameRate*stim.dirsB(1,:)*stim.vel;
        ytot = centerPos(2)+yism(:)'+stim.orgPosB(2,:)+stim.frameIndex/screenInfo.FrameRate*stim.dirsB(2,:)*stim.vel;
    else
        xtot = centerPos(1)+xism(:)'+stim.orgPosB(1,:)+stim.noFigureFrames/screenInfo.FrameRate*stim.dirsB(1,:)*stim.vel;
        ytot = centerPos(2)+yism(:)'+stim.orgPosB(2,:)+stim.noFigureFrames/screenInfo.FrameRate*stim.dirsB(2,:)*stim.vel;

        xtot = xtot+(stim.frameIndex-stim.noFigureFrames)/screenInfo.FrameRate*stim.dirsF(1,:)*stim.vel;
        ytot = ytot+(stim.frameIndex-stim.noFigureFrames)/screenInfo.FrameRate*stim.dirsF(2,:)*stim.vel;
    end

    inds = find((xtot > -stim.dotSize).*(xtot < w+stim.dotSize).*(ytot > -stim.dotSize).*(ytot < h+stim.dotSize));

    for i=1:length(inds)
        x = round(xtot(inds(i)));
        y = round(ytot(inds(i)));


        xs = x+edges;
        ys = y+edges;

        if (xs(1) <= 0) || (xs(end) > size(im,2)) || (ys(1) <= 0) || (ys(end) > size(im,1))
            xinds = find((xs > 0).*(xs <= size(im,2)));
            yinds = find((ys > 0).*(ys <= size(im,1)));

            %im(ys(yinds),xs(xinds)) = im(ys(yinds),xs(xinds)).*(1-dotMask(yinds,xinds)) + dotMask(yinds,xinds)*stim.col;
        else
            if figureMask(y,x)==1 && stim.figureDotIntensity == 255
                im(ys,xs) = im(ys,xs).*(1-dotMask) + dotMask*stim.dotIntensityF(inds(i));
            end
        end
    end

    output = im;
end